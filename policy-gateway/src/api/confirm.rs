//! POST /api/cert-confirm — 确认证书接收（两阶段确认第二步）
//!
//! 客户端收到 CA 签发的证书后，调用此端点确认。
//! 需要 MANAGER_TOKEN 鉴权，防止 request_id 泄露导致未授权确认。

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::EntryStatus;
use crate::AppState;

#[derive(Deserialize)]
pub struct ConfirmRequest {
    pub request_id: String,
    pub token: Option<String>,
    pub hw_id: Option<String>,
    pub hw_platform: Option<String>,
}

#[derive(Serialize)]
pub struct ConfirmResponse {
    pub status: String,
    pub message: String,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfirmRequest>,
) -> Result<Json<ConfirmResponse>, (StatusCode, Json<ConfirmResponse>)> {
    let token = req.token.as_deref().unwrap_or("");
    if !crate::api::manager::check_auth(token) {
        return Err((StatusCode::UNAUTHORIZED, Json(ConfirmResponse {
            status: "error".into(),
            message: "token 无效".into(),
        })));
    }

    // ---- 写锁范围（修改权限表） ----
    let db_sha256 = {
        let mut table = state.auth_table.write().await;
        let entry = table.get_by_request_id(&req.request_id);
        if entry.is_none() {
            return Err((StatusCode::NOT_FOUND, Json(ConfirmResponse {
                status: "error".into(),
                message: "request_id 不存在".into(),
            })));
        }
        let entry = entry.unwrap();
        if entry.status != EntryStatus::PendingConfirm {
            return Err((StatusCode::CONFLICT, Json(ConfirmResponse {
                status: "error".into(),
                message: format!("证书状态为 {:?}，不可确认", entry.status),
            })));
        }

        let sha256 = entry.sha256;
        let _ = entry;

        let entry = table.get_mut(&sha256);
        if let Some(e) = entry {
            e.status = EntryStatus::Active;
            e.bitmap = e.requested_bitmap;
            if let Some(ref hid) = req.hw_id { e.hw_id = Some(hid.clone()); }
            e.hw_platform = req.hw_platform.clone();
        }
        let _ = table.remove_pending(&req.request_id);
        sha256
    }; // 写锁在这里释放

    log::info!("✅ 证书确认: {}", req.request_id);

    // ---- 读锁范围（持久化到 redb） ----
    let sha256_hex = hex::encode(db_sha256);
    let table_r = state.auth_table.read().await;
    if let Some(entry) = table_r.get(&db_sha256) {
        if let Ok(entry_json) = serde_json::to_string(entry) {
            let _ = crate::store::put(&sha256_hex, &entry_json).await;
        }
    }

    Ok(Json(ConfirmResponse {
        status: "active".into(),
        message: "证书已确认，可以正常使用".into(),
    }))
}
