//! policy-gateway — mTLS 网关 + 权限表 + 计算调度
//!
//! Phase 0: 核心 HTTP 服务 + /api/signup + 审批 + 权限表 + GC

mod tls;
mod auth;
mod api;
mod anti_abuse;

use std::sync::Arc;
use tokio::sync::RwLock;
use axum::Router;
use axum::routing::{get, post};

/// 全局共享状态
pub struct AppState {
    pub auth_table: RwLock<auth::AuthTable>,
    pub anti_abuse: RwLock<anti_abuse::AntiAbuse>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info")
    ).init();

    // CLI 模式: 直接执行命令，不启动 HTTP
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return cli_mode(&args).await;
    }

    let state = Arc::new(AppState {
        auth_table: RwLock::new(auth::AuthTable::new()),
        anti_abuse: RwLock::new(anti_abuse::AntiAbuse::new()),
    });

    // 后台 GC 任务（每小时）
    let gc_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let report = gc_state.auth_table.write().await.gc();
            if report.total_removed > 0 {
                log::info!("🧹 GC 清理了 {} 个过期条目，剩余 {}", report.total_removed, report.remaining_entries);
            }
        }
    });

    let app = Router::new()
        .route("/api/signup", post(api::signup::handle))
        .route("/api/signup/status", get(api::status::handle))
        .route("/manager", get(api::manager::handle_page))
        .route("/api/manager/approve", post(api::manager::handle_approve))
        .with_state(state);

    let addr = "0.0.0.0:8443";
    log::info!("🔐 policy-gateway v0.1 — 启动于 {addr}");
    log::info!("   CLI 命令: perm gc, perm list, perm stats");

    // TODO Phase 1: 添加 mTLS (rustls ServerConfig)
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

/// CLI 子命令模式 — SSH 直接调用
///
/// # 用法
/// ```bash
/// policy-gateway perm gc       # 手动触发垃圾回收
/// policy-gateway perm list     # 列出所有权限条目
/// policy-gateway perm stats    # 权限表统计
/// ```
async fn cli_mode(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("perm") => match args.get(2).map(|s| s.as_str()) {
            Some("gc") => {
                let mut table = auth::AuthTable::new();
                // CLI 模式从文件加载权限表（TODO Phase 2: 持久化）
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
                let active = table.list_active().len();
                let compromised = table.list_compromised().len();
                let total = table.len();
                println!("权限表统计:");
                println!("  总条目:     {}", total);
                println!("  活跃:       {}", active);
                println!("  已吊销:     {}", compromised);
                println!("  待审批:     {}", table.list_pending().len());
                println!("  可回收:     {}", total - active);
            }
            _ => {
                eprintln!("用法: policy-gateway perm <gc|list|stats>");
                std::process::exit(1);
            }
        },
        _ => {
            eprintln!("用法: policy-gateway <perm>");
            std::process::exit(1);
        }
    }
}
