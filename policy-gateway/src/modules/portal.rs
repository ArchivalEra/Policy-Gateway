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
        .route("/manager", get(crate::api::manager::handle_page))
        .route("/api/manager/approve", post(crate::api::manager::handle_approve))
        .route("/api/cert-confirm", post(crate::api::confirm::handle))
        .route("/permissions", get(crate::api::permissions::handle_page))
        .route("/api/manager/pending", get(crate::api::manager::handle_pending_json))
        .with_state(state)
}

pub fn portal_name() -> &'static str {
    "core-portal — 认证门户（必需）"
}
