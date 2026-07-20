//! policy-gateway — mTLS 网关 + 权限表 + 计算调度
//!
//! Phase 0: 核心 HTTP 服务 + /api/signup + 审批 + 权限表

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

    let state = Arc::new(AppState {
        auth_table: RwLock::new(auth::AuthTable::new()),
        anti_abuse: RwLock::new(anti_abuse::AntiAbuse::new()),
    });

    let app = Router::new()
        .route("/api/signup", post(api::signup::handle))
        .route("/api/signup/status", get(api::status::handle))
        .route("/manager", get(api::manager::handle_page))
        .route("/api/manager/approve", post(api::manager::handle_approve))
        .with_state(state);

    let addr = "0.0.0.0:8443";
    log::info!("🔐 policy-gateway v0.1 — 启动于 {addr}");

    // TODO Phase 1: 添加 mTLS (rustls ServerConfig)
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
