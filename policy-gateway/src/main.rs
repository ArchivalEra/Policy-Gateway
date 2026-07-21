//! policy-gateway — 模块化网关核心
//!
//! Phase 0.5: 模块化架构，核心只做认证门户 + 上网控制。
//! 计算模块已剥离为独立扩展（见 modules/compute/ 或 Worker 调度）。
//!
//! 启动后:
//!   1. HTTP 服务监听 :8443
//!   2. 门户模块提供 /signup /manager 等端点
//!   3. 后台 GC 每小时运行
//!   4. TODO Phase 1: nftables captive portal 拦截无证设备

mod tls;
mod auth;
mod api;
mod recovery;
mod anti_abuse;
pub mod vm;  // VM — 始终包含，核心组件
pub mod modules;

/// 共享状态别名（api 模块中使用）
pub type AppState = modules::CoreState;

use std::sync::Arc;
use tokio::sync::RwLock;
use axum::Router;

use modules::CoreState;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    // CLI 模式
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return cli_mode(&args).await;
    }

    log::info!("🔐 policy-gateway — 模块化核心");
    log::info!("   核心功能: 证书认证 + 上网控制");
    log::info!("   计算模块: 已剥离（通过 Worker 调度或部署 compute-daemon）");

    // 生成/加载 CA 密钥对
    let (ca_key, ca_cert) = match std::env::var("CA_KEY_PEM") {
        Ok(k) => (k, std::env::var("CA_CERT_PEM").unwrap_or_default()),
        Err(_) => crate::tls::generate_ca().unwrap_or_else(|| {
            log::warn!("⚠️ CA 密钥生成失败，使用临时密钥");
            ("temp-key".into(), "temp-cert".into())
        }),
    };
    log::info!("🔑 CA 密钥 {}", if std::env::var("CA_KEY_PEM").is_ok() {"已加载"} else {"已生成"});

    // 核心状态
    let core = Arc::new(CoreState {
        auth_table: Arc::new(RwLock::new(auth::AuthTable::new())),
        anti_abuse: Arc::new(RwLock::new(anti_abuse::AntiAbuse::new())),
        ca_key_pem: ca_key,
        ca_cert_pem: ca_cert,
    });

    // 首次启动检测
    {
        let table = core.auth_table.read().await;
        if table.is_empty() {
            log::warn!("🆕 首次启动 — 权限表为空");
            // 尝试加载 seed.json
            let seed_paths = [
                "/etc/config/policy-gateway.seed.json",
                "/etc/policy-gateway/seed.json",
                "/usr/share/policy-gateway/seed.json",
                "./deploy/seed.json",
            ];
            let mut seeded = false;
            for path in &seed_paths {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(seed) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(entries) = seed.get("entries").and_then(|v| v.as_array()) {
                            let mut table_w = core.auth_table.write().await;
                            for entry in entries {
                                if let (Some(sha), Some(hostname), Some(bitmap), Some(status)) = (
                                    entry.get("sha256").and_then(|v| v.as_str()),
                                    entry.get("hostname").and_then(|v| v.as_str()),
                                    entry.get("bitmap").and_then(|v| v.as_u64()),
                                    entry.get("status").and_then(|v| v.as_str()),
                                ) {
                                    if status == "active" && bitmap > 0 {
                                        if let Ok(sha_bytes) = hex::decode(sha) {
                                            let mut sha_arr = [0u8; 32];
                                            if sha_bytes.len() == 32 {
                                                sha_arr.copy_from_slice(&sha_bytes);
                                                table_w.add_pending(sha_arr, hostname.to_string(), "__seed__".into(), bitmap as u64);
                                                table_w.approve("__seed__", bitmap as u64);
                                                log::info!("   ✅ 预置根证书: {} (bitmap={:x})", hostname, bitmap);
                                                seeded = true;
                                            }
                                        }
                                    }
                                }
                            }
                            if seeded {
                                log::info!("   📋 已从 {} 加载 {} 个条目", path, entries.len());
                            }
                        }
                    }
                }
            }
            if !seeded {
                log::warn!("   ⚠️  未找到 seed.json");
                log::warn!("   首次使用请运行: policy-gateway init");
                log::warn!("    或参考 deploy/bootstrap.sh 生成根证书");
            }
        }
    }

    // 前台: HTTP API
    let app = Router::new()
        .merge(modules::portal::portal_router(core.clone()));

    // 后台: 每小时 GC
    let gc_core = core.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let report = gc_core.auth_table.write().await.gc();
            if report.total_removed > 0 {
                log::info!("🧹 GC 清理了 {} 个过期条目，剩余 {}", report.total_removed, report.remaining_entries);
            }
        }
    });

    let addr = "0.0.0.0:8443";
    log::info!("🌐 监听 {addr} — 设备可通过此端口访问 /signup");
    log::info!("   设备无证书时只能访问 /signup（由 nftables REDIRECT 强制）");

    // TODO Phase 1: 添加 mTLS + nftables captive portal
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// CLI 子命令
async fn cli_mode(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("perm") => match args.get(2).map(|s| s.as_str()) {
            Some("gc") => {
                let mut table = auth::AuthTable::new();
                let report = table.gc();
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            }
            Some("list") => {
                let table = auth::AuthTable::new();
                for entry in table.iter() {
                    println!("{}  {:x}  {}", 
                        hex::encode(entry.sha256),
                        entry.bitmap,
                        entry.hostname,
                    );
                }
                println!("总计: {} 条目", table.len());
            }
            Some("stats") => {
                let table = auth::AuthTable::new();
                println!("权限表统计:");
                println!("  总条目:     {}", table.len());
                println!("  活跃:       {}", table.list_active().len());
                println!("  已吊销:     {}", table.list_compromised().len());
                println!("  待审批:     {}", table.list_pending().len());
            }
            _ => {
                eprintln!("用法: policy-gateway perm <gc|list|stats>");
                std::process::exit(1);
            }
        },
        Some("vm") => { vm::cli(&args[1..]); }
        Some("init") => {
            println!("🔐 policy-gateway 首次设置");
            println!();
            println!("这个命令会生成根证书和初始配置。");
            println!("如果你没有 deploy/seed.json，请先运行 bootstrap.sh。");
            println!();
            println!("用法: policy-gateway init");
            println!("      然后访问 http://<router-ip>:8443/manager?token=<token>");
            println!();
            println!("首次启动说明:");
            println!("  1. 确保 seed.json 和根证书已在 /etc/config/ 目录");
            println!("  2. 启动服务后访问 /manager");
            println!("  3. 用根证书登录，即可审批其他设备");
        }
        Some("module") => {
            println!("📦 模块系统 v0.1");
            println!("  核心: portal（认证门户，必需）");
            println!("  扩展: compute（计算调度，已剥离）");
            println!("  加载路径: /mnt/usb/modules/<name>/module.toml");
            println!("  对象存储: Worker R2 / Oracle S3 兼容");
        }
        _ => {
            eprintln!("用法: policy-gateway <perm|module|init>");
            std::process::exit(1);
        }
    }
}
