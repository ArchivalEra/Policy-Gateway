//! POST /api/cert-confirm — 确认证书接收（两阶段确认第二步）
//!
//! 客户端收到 CA 签发的证书后，调用此端点确认。
//! 确认后证书状态从 PendingConfirm → Active。
//! 未确认的证书 60 天后自动清理。

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::auth::EntryStatus;

#[derive(Deserialize)]
pub struct ConfirmRequest {
    pub request_id: String,
    /// 硬件 ID（可选，确认时补充）
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

    // 更新为 Active
    // 由于我们使用 HashMap，需要先获取 sha256 再 approve
    let sha256 = entry.sha256;
    drop(entry);
    // FIXME: 添加 confirm 方法到 AuthTable
    // 临时方案: 手动修改状态
    let entry = table.get_mut(&sha256);
    if let Some(e) = entry {
        e.status = EntryStatus::Active;
        // 更新硬件 ID（如果提供）
        if let Some(ref hid) = req.hw_id {
            e.hw_id = Some(hid.clone());
        }
        e.hw_platform = req.hw_platform.clone();
    }

    log::info!("✅ 证书确认: {}", req.request_id);

    Ok(Json(ConfirmResponse {
        status: "active".into(),
        message: "证书已确认，可以正常使用".into(),
    }))
}
