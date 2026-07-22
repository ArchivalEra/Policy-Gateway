//! 门户模块 — 核心功能：认证 + 审批 + 上网控制
//!
//! 这是系统唯一必需的模块。没有它，设备连 signup 都看不到。
//! 它提供:
//!   - /api/signup      证书申请
//!   - /api/signup/status  状态查询
//!   - /manager         管理员审批面板
//!   - /api/manager/approve 审批操作

use axum::Router;
use axum::routing::{get, post};

use super::CoreState;

pub fn portal_router(state: std::sync::Arc<CoreState>) -> Router {
    Router::new()
        .route("/api/signup", post(crate::api::signup::handle))
        .route("/api/signup/status", get(crate::api::status::handle))
        .route("/signup/status", get(crate::api::status::handle_html))
        .route("/manager", get(crate::api::manager::handle_page))
        .route("/api/manager/approve", post(crate::api::manager::handle_approve))
        .route("/api/cert-confirm", post(crate::api::confirm::handle))
        .route("/permissions", get(crate::api::permissions::handle_page))
        .route("/api/manager/pending", get(crate::api::manager::handle_pending_json))
        .route("/api/help", get(crate::api::help::handle))
        .route("/healthz", get(healthz))
        .route("/signup", get(crate::api::signup::handle_form))
        .route("/", get(root_handler))
        .with_state(state)
}

pub fn portal_name() -> &'static str {
    "core-portal — 认证门户（必需）"
}

/// GET /healthz — 健康检查
pub async fn healthz() -> axum::response::Json<serde_json::Value> {
    axum::response::Json(serde_json::json!({
        "status": "ok",
        "service": "policy-gateway",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// GET / — 简洁首页
pub async fn root_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(r#"<!DOCTYPE html>
<html lang="zh"><head><meta charset="UTF-8"><title>policy-gateway</title>
<style>body{font-family:sans-serif;max-width:600px;margin:auto;padding:40px;text-align:center}
a{display:block;padding:12px;margin:8px;background:#06c;color:#fff;border-radius:6px;text-decoration:none;font-size:18px}
a:hover{background:#058}</style></head>
<body>
<h1>🔐 policy-gateway</h1>
<p>证书管理网关</p>
<a href="/signup">📜 申请证书</a>
<a href="/manager">🔑 管理面板</a>
<a href="/permissions">📋 权限表</a>
<a href="/api/help">📖 接入教程</a>
</body></html>"#)
}
