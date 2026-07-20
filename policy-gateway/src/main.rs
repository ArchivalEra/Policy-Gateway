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
mod anti_abuse;
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

    // 核心状态
    let core = Arc::new(CoreState {
        auth_table: Arc::new(RwLock::new(auth::AuthTable::new())),
        anti_abuse: Arc::new(RwLock::new(anti_abuse::AntiAbuse::new())),
    });

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
        Some("module") => {
            println!("📦 模块系统 v0.1");
            println!("  核心: portal（认证门户，必需）");
            println!("  扩展: compute（计算调度，已剥离）");
            println!("  加载路径: /mnt/usb/modules/<name>/module.toml");
            println!("  对象存储: Worker R2 / Oracle S3 兼容");
        }
        _ => {
            eprintln!("用法: policy-gateway <perm|module>");
            std::process::exit(1);
        }
    }
}
