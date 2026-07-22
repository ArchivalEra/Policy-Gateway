//! 门户模块 — 核心功能：认证 + 审批 + 上网控制
//!
//! 这是系统唯一必需的模块。没有它，设备连 signup 都看不到。
//! Phase 2.9: 默认仅 JSON API。HTML 页面可通过 "frontend" feature 启用。
//! 它提供:
//!   - /api/signup      证书申请 (JSON)
//!   - /api/signup/status  状态查询 (JSON)
//!   - /api/manager/pending  待审批列表 (JSON)
//!   - /api/manager/approve 审批操作 (JSON)

use axum::Router;
use axum::routing::{get, post};

use super::CoreState;

pub fn portal_router(state: std::sync::Arc<CoreState>) -> Router {
    #[allow(unused_mut)]
    let mut router = Router::new()
        .route("/api/signup", post(crate::api::signup::handle))
        .route("/api/signup/status", get(crate::api::status::handle))
        .route("/api/manager/approve", post(crate::api::manager::handle_approve))
        .route("/api/cert-confirm", post(crate::api::confirm::handle))
        .route("/api/manager/pending", get(crate::api::manager::handle_pending_json))
        .route("/api/help", get(crate::api::help::handle))
        .route("/permissions", get(crate::api::permissions::handle_page))
        .route("/healthz", get(healthz));

    #[cfg(feature = "frontend")]
    if state.serve_html {
        router = router
            .route("/signup", get(crate::api::signup::handle_form))
            .route("/signup/status", get(crate::api::status::handle_html))
            .route("/manager", get(crate::api::manager::handle_page))
            .route("/", get(root_handler));
    }
    #[cfg(not(feature = "frontend"))]
    if state.serve_html {
        log::warn!("serve_html=true 但编译时未启用 frontend feature");
    }

    router.with_state(state)
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

#[cfg(feature = "frontend")]
/// GET / — 简洁首页
pub async fn root_handler() -> axum::response::Html<&'static str> {
    axum::response::Html(r#"<!DOCTYPE html>
<html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>policy-gateway</title>
<style>
*{box-sizing:border-box}
body{font-family:-apple-system,system-ui,sans-serif;max-width:600px;margin:auto;padding:20px;text-align:center;background:#f5f5f5;color:#333}
.card{background:#fff;border-radius:12px;padding:20px;margin:12px 0;box-shadow:0 1px 3px rgba(0,0,0,.1)}
a{display:block;padding:14px;margin:8px;background:#0066cc;color:#fff;border-radius:8px;text-decoration:none;font-size:18px;font-weight:500}
a:hover{background:#0052a3}
a.secondary{background:#6c757d;font-size:14px}
@media(max-width:480px){body{padding:12px}a{font-size:16px;padding:12px}}</style></head>
<body>
<div class="card">
<h1>🔐 policy-gateway</h1>
<p style="font-size:14px;color:#666">证书管理网关</p>
</div>
<div class="card" style="padding:12px">
<a href="/signup">📜 申请证书</a>
<a href="/manager">🔑 管理面板</a>
<a href="/permissions">📋 权限表</a>
<a href="/api/help" class="secondary">📖 接入教程</a>
</div>
</body></html>"#)
}
