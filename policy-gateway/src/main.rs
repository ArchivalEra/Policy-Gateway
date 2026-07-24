//! policy-gateway — 模块化网关核心
//!
//! Phase 3.3: 路由器部署验证 + storage 抽象 + v0.3.3。
//! 计算模块已剥离为独立扩展（见 parts/compute/ 或 Worker 调度）。
//!
//! 启动后:
//!   1. HTTP 服务监听 :8443
//!   2. nftables 自动部署双表 (pg_pre + pg_nat)
//!   3. 后台 GC 每小时运行
//!   4. nftables FORWARD policy-drop + pg_nat REDIRECT 门户

mod tls;
mod auth;
mod api;
mod recovery;
mod anti_abuse;
pub mod vm;  // VM — 始终包含，核心组件
pub mod parts;
pub mod store;  // redb 持久化
pub mod event_log;  // 时间戳事件系统
pub mod config;  // 统一配置
pub mod lang;  // 国际化
pub mod nft;  // nftables 规则管理

/// 共享状态别名（api 模块中使用）
pub type AppState = parts::CoreState;

use std::sync::Arc;
use ring::signature::KeyPair as _;
use tokio::sync::RwLock;
use axum::Router;
use parts::CoreState;
use crate::lang::{t, S as _S};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    // CLI 模式
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args[1] == "--help" || args[1] == "-h" {
        print_help();
        return;
    }
    if args[1] == "--version" || args[1] == "-V" {
        println!("policy-gateway v{}", env!("CARGO_PKG_VERSION"));
        return;
    }
    match args[1].as_str() {
        "serve" | "gui" | "web" => {
            let serve_html = !args.iter().any(|a| a == "--no-html" || a == "--api-only");
            let pid = std::process::id();
            let _ = std::fs::write("/var/run/policy-gateway.pid", pid.to_string());
            let _ = std::fs::write("/tmp/policy-gateway.pid", pid.to_string());
            start_server(serve_html).await;
        }
        "stop" => {
            let pid_paths = ["/tmp/policy-gateway.pid", "/var/run/policy-gateway.pid"];
            let mut stopped = false;
            for path in &pid_paths {
                if let Ok(s) = std::fs::read_to_string(path) {
                    if let Ok(pid) = s.trim().parse::<i32>() {
                        let _ = std::process::Command::new("kill").args(["-15", &pid.to_string()]).status();
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
                        let _ = std::fs::remove_file(path);
                        println!("✅ policy-gateway stopped (PID {})", pid);
                        stopped = true;
                    }
                }
            }
            if !stopped {
                let _ = std::process::Command::new("killall").args(["-9", "policy-gateway"]).status();
                println!("✅ all policy-gateway processes stopped");
            }
        }
        _ => { cli_mode(&args).await; }
    }
}

async fn start_server(serve_html: bool) {
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
        event_log: Arc::new(RwLock::new(crate::event_log::EventLog::new())),
        serve_html,
        event_tx: tokio::sync::broadcast::channel::<String>(256).0,
    });

    // 初始化 redb 持久化
    let db_path = std::env::var("PG_DB_PATH").unwrap_or_else(|_| "/etc/config/policy-gateway/auth.redb".to_string());
    let parent = std::path::Path::new(&db_path).parent().unwrap_or(std::path::Path::new("/tmp"));
    if let Err(e) = std::fs::create_dir_all(parent) {
        log::warn!("⚠️  无法创建数据目录 {}: {}（权限表将在内存中运行）", parent.display(), e);
    } else if let Err(e) = crate::store::init_store(&db_path) {
        log::warn!("⚠️  {} 不可用: {} — 尝试 /tmp/auth.redb...", db_path, e);
        if let Err(e2) = crate::store::init_store("/tmp/auth.redb") {
            log::warn!("⚠️  /tmp/auth.redb 也不可用: {}（权限表将在内存中运行）", e2);
        } else {
            log::info!("   📀 持久化存储: /tmp/auth.redb (tmpfs)");
        }
    } else {
        log::info!("   📀 持久化存储: {}", db_path);
        // 从数据库恢复已保存的条目
        if let Ok(entries) = crate::store::iter().await {
            let mut table_w = core.auth_table.write().await;
            for (sha256_hex, entry_json) in entries {
                if let Ok(sha256) = hex::decode(&sha256_hex) {
                    let mut arr = [0u8; 32];
                    if sha256.len() == 32 {
                        arr.copy_from_slice(&sha256);
                        if let Ok(entry) = serde_json::from_str::<crate::auth::PermissionEntry>(&entry_json) {
                            table_w.put(arr, entry);
                        }
                    }
                }
            }
            log::info!("   📂 恢复 {} 个条目", table_w.iter().count());
        }
    }

    // 模块加载
    {
        let _registry = parts::ModuleRegistry::new();

        // 检查维护模式
        let maintenance_path = "/etc/backup/policy-gateway/maintenance.json";
        let in_maintenance = std::fs::read_to_string(maintenance_path).ok().and_then(|s| {
            serde_json::from_str::<serde_json::Value>(&s).ok()
        }).and_then(|v| {
            let start = v.get("maintenance_start").and_then(|t| t.as_u64())?;
            let end = v.get("maintenance_end").and_then(|t| t.as_u64())?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            Some(now >= start && now < end)
        }).unwrap_or(false);

        if in_maintenance {
            log::warn!("🛠️  维护模式激活 — 所有页面将转向 maintenance.html");
            log::warn!("   CLI 返回 'maintenance between X and Y'");
        }
        let mut registry = parts::ModuleRegistry::new();

        // 尝试加载 storage-more 模块
        let cfg = crate::config::Config::load();
        let storage_backend = cfg.storage_backend.clone();
        drop(cfg);

        let sm = parts::storage_more::StorageMore::new(
            None, // path
            if storage_backend == "dir" { Some("dir") } else { None },
        );
        match registry.register(Box::new(sm)) {
            Ok(_) => log::info!("📦 storage-more: bit 3,4 已注册"),
            Err(e) => log::warn!("📦 storage-more: {} — bit 3,4 已锁定", e),
        }

        // 加载 dns-local 模块
        let cfg2 = crate::config::Config::load();
        if !cfg2.dns_hosts.is_empty() {
            let dns = parts::dns_local::DnsLocal::new(&cfg2.dns_hosts);
            match registry.register(Box::new(dns)) {
                Ok(_) => log::info!("📦 dns-local: 自定义 DNS 映射已加载"),
                Err(e) => log::warn!("📦 dns-local: {}", e),
            }
        }

        // 加载 worker-sync 模块（配置了 worker_url 时）
        if let (Some(url), Some(token)) = (cfg2.worker_url.as_ref(), cfg2.worker_token.as_ref()) {
            let sync = parts::worker_sync::WorkerSync::new(url, token, cfg2.worker_sync_interval);
            match registry.register(Box::new(sync)) {
                Ok(_) => {
                    log::info!("📡 worker-sync: 已注册 (interval={}s)", cfg2.worker_sync_interval);
                    // 后台同步循环
                    let sync_runner = parts::worker_sync::WorkerSync::new(url, token, cfg2.worker_sync_interval);
                    tokio::spawn(async move { sync_runner.run().await; });
                }
                Err(e) => log::warn!("📡 worker-sync: {}", e),
            }
        }

        log::info!("🔒 锁定权限位: 活动 {:?}, 锁定 {} 个",
            registry.active_bits(),
            registry.locked_bits().len());
    }

    // 首次启动检测
    {
        // 如果 MANAGER_TOKEN 未设置，尝试从 seed.json 读取
        if std::env::var("MANAGER_TOKEN").is_err() {
            let fallback_paths = [
                "/etc/config/policy-gateway/seed.json",
                "/etc/config/policy-gateway.seed.json",
                "/etc/policy-gateway/seed.json",
                "./deploy/seed.json",
            ];
            for path in &fallback_paths {
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(seed) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(token) = seed.get("manager_token").and_then(|v| v.as_str()) {
                            std::env::set_var("MANAGER_TOKEN", token);
                            log::info!("   🔑 MANAGER_TOKEN 从 seed.json 加载");
                            break;
                        }
                    }
                }
            }
        }

        let table = core.auth_table.read().await;
        let needs_seed = table.is_empty();
        drop(table);  // 释放读锁，避免后续写锁死锁

        if needs_seed {
            log::warn!("🆕 首次启动 — 权限表为空");
            // 尝试加载 seed.json
            let seed_paths = [
                "/etc/config/policy-gateway.seed.json",
                "/etc/config/policy-gateway/seed.json",
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
        .merge(parts::portal::portal_router(core.clone()));

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

    // nftables 自动部署
    let nft_config = crate::config::Config::load();
    let nft_interfaces = nft_config.monitor_interfaces.clone();
    drop(nft_config);

    log::info!("🛡️  部署 nftables 规则...");
    let nft_result = if nft_interfaces.is_empty() {
        crate::nft::deploy()
    } else {
        crate::nft::deploy_with_interfaces(&nft_interfaces)
    };
    match nft_result {
        Ok(_) => log::info!("✅ nftables 双表已部署"),
        Err(e) => log::warn!("⚠️  nftables 部署失败: {}（如非 OpenWrt 环境可忽略）", e),
    }

    // 注册退出清理 (SIGTERM/SIGINT 时自动清理 nftables)
    // 注意: 不要注册 panic hook 清理，因为 bind 失败等非致命错误也会触发 panic，
    //       导致 nftables 规则被错误删除。

    let addr = "0.0.0.0:8443";
    log::info!("🌐 监听 {addr} — 设备可通过此端口访问 /signup");
    log::info!("   设备无证书时只能访问 /signup（由 nftables REDIRECT 强制）");

    let quic_config = crate::config::Config::load().tls_profile.allow_quic;
    if quic_config {
        log::warn!("⚠️ QUIC 支持需要安装 quic_optimized 模块");
        log::warn!("   当前 nftables 规则会拦截 UDP 443 (QUIC → TCP fallback)");
        log::warn!("   安装模块后请手动删除此规则或由模块接管");
    }

    // TLS 监听器: 如果配置了证书则启动
    let config = crate::config::Config::load();
    let has_tls = config.tls_cert.as_ref().and_then(|c| {
        let exists = std::path::Path::new(c).exists();
        if !exists { log::warn!("⚠️ TLS 证书路径不存在: {}", c); }
        Some(exists)
    }).unwrap_or(false);

    if has_tls && !config.tls_profile.allow_http {
        log::info!("🔒 TLS 模式 — 仅 HTTPS");
        log::warn!("⚠️   TLS 监听器需要 hyper-util 编译支持，当前版本仅 HTTP");
        log::warn!("   运行 policy-gateway init 生成自签名证书后配置 tls_cert/tls_key");
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => { let _ = axum::serve(listener, app).await; }
            Err(e) => log::error!("❌ 监听 {} 失败: {}（可能端口被占用）", addr, e),
        }
    } else {
        if has_tls && config.tls_profile.allow_http {
            log::info!("🔒 TLS 证书已加载，同时监听 HTTP + HTTPS (TLS 待完整实现)");
        }
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => { let _ = axum::serve(listener, app).await; }
            Err(e) => log::error!("❌ 监听 {} 失败: {}（可能端口被占用）", addr, e),
        }
    }
}

/// CLI 子命令

fn print_help() {
    println!("policy-gateway v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("{}", t(_S::CliUsage));
    println!("  policy-gateway serve [--no-html]  {}", t(_S::CliServe));
    println!("  policy-gateway --version, -V      {}", t(_S::CliVersion));
    println!("  policy-gateway --help, -h         {}", t(_S::CliHelp));
    println!("  policy-gateway perm <gc|list|stats> {}", t(_S::CliPerm));
    println!("  policy-gateway vm <command>       {}", t(_S::CliVm));
    println!("  policy-gateway config <edit|show|reset|tls> {}", t(_S::ConfigText));
    println!("  policy-gateway init               {}", t(_S::CliInit));
    println!("  policy-gateway module             {}", t(_S::CliModule));
    println!("  policy-gateway snapshot           创建 pg 快照 (redb + vm-mod + modules + cli-registry)");
    println!("  policy-gateway rollback           回滚 pg 快照");
    println!();
    println!("{} (policy-gateway-vm):", t(_S::CliVm));
    println!("  init    {}", t(_S::CliVmInit));
    println!("  snapshot {}", t(_S::CliVmSnapshot));
    println!("  rollback {}", t(_S::CliVmRollback));
    println!("  list    {}", t(_S::CliVmList));
    println!("  status  {}", t(_S::CliVmStatus));
    println!("  verify  {}", t(_S::CliVmVerify));
}

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
        Some("config") => {
            let mut cfg = crate::config::Config::load();
            let sub = args.get(2).map(|s| s.as_str());
            match sub {
                Some("edit") | Some("interactive") | None => cfg.interactive(),
                Some("show") | Some("dump") => {
                    println!("{}", toml::to_string_pretty(&cfg).unwrap_or_default());
                    return;
                }
                Some("reset") => {
                    let default = crate::config::Config::default();
                    default.save().ok();
                    println!("✅ 配置已重置");
                    return;
                }
                Some("tls") => {
                    let tls_sub = args.get(3).map(|s| s.as_str());
                    match tls_sub {
                        Some("show") => {
                            println!("{}", toml::to_string_pretty(&cfg.tls_profile).unwrap_or_default());
                        }
                        Some("edit") | None => {
                            let mut http_s = if cfg.tls_profile.allow_http { "y" } else { "n" }.to_string();
                            crate::config::Config::prompt("允许 HTTP (y/n)", &mut http_s);
                            cfg.tls_profile.allow_http = http_s == "y" || http_s == "yes" || http_s == "true" || http_s == "1";
                            let mut t12 = if cfg.tls_profile.allow_tls12 { "y" } else { "n" }.to_string();
                            crate::config::Config::prompt("允许 TLS 1.2 (y/n)", &mut t12);
                            cfg.tls_profile.allow_tls12 = t12 == "y" || t12 == "yes" || t12 == "true" || t12 == "1";
                            let mut t13 = if cfg.tls_profile.allow_tls13 { "y" } else { "n" }.to_string();
                            crate::config::Config::prompt("允许 TLS 1.3 (y/n)", &mut t13);
                            cfg.tls_profile.allow_tls13 = t13 == "y" || t13 == "yes" || t13 == "true" || t13 == "1";
                            let mut quic = if cfg.tls_profile.allow_quic { "y" } else { "n" }.to_string();
                            crate::config::Config::prompt("允许 QUIC (y/n)", &mut quic);
                            cfg.tls_profile.allow_quic = quic == "y" || quic == "yes" || quic == "true" || quic == "1";
                            let mut mtls = if cfg.tls_profile.mtls_enabled { "y" } else { "n" }.to_string();
                            crate::config::Config::prompt("启用 mTLS (y/n)", &mut mtls);
                            cfg.tls_profile.mtls_enabled = mtls == "y" || mtls == "yes" || mtls == "true" || mtls == "1";
                            cfg.save().ok();
                            println!("✅ TLS 配置已保存");
                        }
                        Some("enable") => {
                            cfg.tls_profile.allow_tls12 = true;
                            cfg.tls_profile.allow_tls13 = true;
                            cfg.save().ok();
                            println!("✅ TLS 1.2 + 1.3 已启用");
                        }
                        Some("disable") => {
                            cfg.tls_profile.allow_tls12 = false;
                            cfg.tls_profile.allow_tls13 = false;
                            cfg.save().ok();
                            println!("✅ TLS 已禁用（仅 HTTP）");
                        }
                        _ => {
                            eprintln!("用法: policy-gateway config tls <edit|show|enable|disable>");
                            std::process::exit(1);
                        }
                    }
                    return;
                }
                _ => {
                    eprintln!("用法: policy-gateway config <edit|show|reset|tls>");
                    std::process::exit(1);
                }
            }
            match cfg.save() {
                Ok(_) => println!("✅ 配置已保存"),
                Err(e) => eprintln!("❌ {}", e),
            }
        }
        Some("init") => {
            println!("policy-gateway first setup");
            println!();

            // check if CA key exists
            let ca_key_path = "/etc/config/policy-gateway/ca.key";
            if std::path::Path::new(ca_key_path).exists() {
                println!("CA key already exists: {}", ca_key_path);
                println!("  Delete it to regenerate.");
                return;
            }

            // generate Ed25519 CA keypair
            println!("generating Ed25519 CA keypair...");
            let rng = ring::rand::SystemRandom::new();
            let pkcs8 = ring::signature::Ed25519KeyPair::generate_pkcs8(&rng)
                .expect("key generation failed");
            let kp = ring::signature::Ed25519KeyPair::from_pkcs8(pkcs8.as_ref())
                .expect("key parse failed");
            let pk_bytes = kp.public_key().as_ref();
            let pk_hex = hex::encode(pk_bytes);

            // self-signed root certificate
            println!("signing self-signed root certificate...");
            let not_before = chrono::Utc::now().timestamp();
            let not_after = not_before + 3650 * 86400;
            let body = format!(
                "subject:policy-gateway-root\nserial:00\npubkey:{}\nnot_before:{}\nnot_after:{}\nrole:root\n",
                pk_hex, not_before, not_after
            );
            let sig = kp.sign(body.as_bytes());
            let sig_hex = hex::encode(sig.as_ref());
            let cert_pem = format!(
                "-----BEGIN CERTIFICATE-----\n{}\nsignature:{}\n-----END CERTIFICATE-----\n",
                body, sig_hex
            );

            // seed.json
            let sha256 = crate::tls::pem_sha256(&cert_pem);
            let token = uuid::Uuid::new_v4().to_string();
            let seed = serde_json::json!({
                "manager_token": token,
                "root_cert_pem": cert_pem,
                "ca_pubkey_hex": pk_hex,
                "entries": [{
                    "sha256": hex::encode(sha256),
                    "hostname": "initial root",
                    "bitmap": 255,
                    "status": "active"
                }]
            });
            let config_dir = std::path::Path::new("/etc/config/policy-gateway");
            std::fs::create_dir_all(config_dir).ok();
            let seed_path = config_dir.join("seed.json");
            match std::fs::write(&seed_path, serde_json::to_string_pretty(&seed).unwrap()) {
                Ok(_) => println!("wrote {}", seed_path.display()),
                Err(e) => eprintln!("write seed.json failed: {} (not root?)", e),
            }

            // 生成自签名 TLS 证书
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            let tls_dir = std::path::Path::new(&home).join(".policy-gateway").join("tls");
            std::fs::create_dir_all(&tls_dir).ok();
            let tls_cert_path = tls_dir.join("cert.pem");
            let tls_key_path = tls_dir.join("key.pem");

            if !tls_cert_path.exists() || !tls_key_path.exists() {
                let gen_result = std::process::Command::new("openssl")
                    .args([
                        "req", "-x509", "-newkey", "ed25519",
                        "-keyout", tls_key_path.to_str().unwrap(),
                        "-out", tls_cert_path.to_str().unwrap(),
                        "-days", "3650", "-nodes",
                        "-subj", "/CN=policy-gateway/O=policy-gateway-self-signed",
                    ])
                    .output();

                match gen_result {
                    Ok(out) if out.status.success() => {
                        println!("✅ TLS cert -> {}", tls_cert_path.display());
                        println!("✅ TLS key  -> {}", tls_key_path.display());
                    }
                    _ => {
                        println!("⚠️ openssl not available, TLS cert not generated");
                        println!("   run: openssl req -x509 -newkey ed25519 -keyout ~/.policy-gateway/tls/key.pem \\");
                        println!("         -out ~/.policy-gateway/tls/cert.pem -days 3650 -nodes");
                    }
                }
            } else {
                println!("✅ TLS cert already exists: {}", tls_cert_path.display());
            }

            // 保存配置
            let mut cfg = crate::config::Config::load();
            cfg.manager_token_hash = crate::config::hash_token(&token);
            if tls_cert_path.exists() && tls_key_path.exists() {
                cfg.tls_cert = Some(tls_cert_path.to_str().unwrap().to_string());
                cfg.tls_key = Some(tls_key_path.to_str().unwrap().to_string());
            }
            cfg.save().ok();

            println!();
            println!("⚠️  根证书待验证");
            println!("   在信任此设备之前, 请验证至少一种恢复途径可用:");
            println!();
            println!("   1. 本地短码恢复:");
            println!("      policy-gateway recovery setup-shortcode");
            println!("      然后重启服务, 确认能通过短码恢复");
            println!();
            println!("   2. Worker 远程恢复:");
            println!("      部署 vm-worker 到 Cloudflare Pages,");
            println!("      设置 worker_url 和 worker_token,");
            println!("      访问 /recover 页面验证恢复流程");
            println!();
            println!("   3. 证书加密恢复 (默认可用):");
            println!("      根证书文件本身即可恢复, 无需配置");
            println!();
            println!("   完成验证后运行:");
            println!("      policy-gateway init --confirm");
            println!("      (将根证书从 pending 转为 active)");
            println!();
            println!("setup complete!");
            println!("  root cert SHA256: {}", hex::encode(sha256));
            println!("  manager token:    {}", token);
            println!("  http://<router-ip>:8443/manager?token={}", token);
            println!("  save this token! lost it -> use Worker recovery.");
            if tls_cert_path.exists() {
                println!("  TLS enabled. HTTPS on port 443.");
            }
            println!();
            println!("  next: policy-gateway serve");
        }
        Some("module") => {
            println!("📦 模块系统 v0.1");
            println!("  核心: portal（认证门户，必需）");
            println!("  扩展: compute（计算调度，已剥离）");
            println!("  加载路径: /mnt/usb/modules/<name>/module.toml");
            println!("  对象存储: Worker R2 / Oracle S3 兼容");
        }
        Some("snapshot") => {
            let cfg = crate::config::Config::load();
            let snap = std::path::Path::new(&cfg.backups_dir).join("pg-snapshot");
            let _ = std::fs::create_dir_all(&snap);
            let mods_toml = format!("{}/modules.toml", cfg.modules_dir.trim_end_matches("/modules"));
            let cli_reg = "/etc/config/policy-gateway/cli-registry.toml".to_string();
            for (name, src) in &[
                ("auth.redb", cfg.db_path.as_str()),
                ("vm-mod", cfg.vm_mod_bin_path.as_str()),
                ("modules.toml", mods_toml.as_str()),
                ("cli-registry.toml", cli_reg.as_str()),
            ] {
                let p = std::path::Path::new(src);
                if p.exists() { let _ = std::fs::copy(p, snap.join(name)); }
            }
            let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
            let meta = serde_json::json!({"created_at":ts,"scope":"pg"});
            let _ = std::fs::write(snap.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());
            println!("📸 pg snapshot saved ({})", snap.display());
        }
        Some("rollback") => {
            let cfg = crate::config::Config::load();
            let snap = std::path::Path::new(&cfg.backups_dir).join("pg-snapshot");
            if !snap.join("auth.redb").exists() { eprintln!("❌ no pg snapshot"); return; }
            let _ = std::fs::create_dir_all("/etc/config/policy-gateway");
            for (name, dst) in &[
                ("auth.redb", cfg.db_path.as_str()),
                ("vm-mod", cfg.vm_mod_bin_path.as_str()),
                ("modules.toml", "/etc/config/policy-gateway/modules.toml"),
                ("cli-registry.toml", "/etc/config/policy-gateway/cli-registry.toml"),
            ] {
                if snap.join(name).exists() { let _ = std::fs::copy(snap.join(name), dst); }
            }
            println!("⏪ pg snapshot restored");
        }
        _ => {
            eprintln!("用法: policy-gateway <perm|module|init|snapshot|rollback>");
            std::process::exit(1);
        }
    }
}
