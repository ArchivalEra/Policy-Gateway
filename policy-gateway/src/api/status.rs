//! GET /api/signup/status — 查询申请状态
//!
//! 支持两种查法：
//!   ?id=<request_id>     — 通过申请 ID 查询
//!   ?sha256=<hex>        — 通过证书 SHA256 查询

use axum::extract::{State, Query};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;

#[derive(Deserialize)]
pub struct StatusQuery {
    pub id: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub hostname: Option<String>,
    pub bitmap: Option<String>,
    pub request_id: Option<String>,
    pub reason: Option<String>,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StatusQuery>,
) -> Json<StatusResponse> {
    let table = state.auth_table.read().await;

    let entry = if let Some(id) = &q.id {
        table.get_by_request_id(id)
    } else if let Some(hex_str) = &q.sha256 {
        let sha256 = match hex_decode(hex_str) { Ok(h) => h, Err(_) => return Json(StatusResponse { status: "invalid_sha256".into(), hostname: None, bitmap: None, request_id: None, reason: None }) };
        table.get(&sha256)
    } else {
        None
    };

    match entry {
        Some(e) => Json(StatusResponse {
            status: e.status.to_string(),
            hostname: Some(e.hostname.clone()),
            bitmap: Some(bitmap_to_hex(e.bitmap & crate::auth::valid_bits_mask())),
            request_id: None, // 不暴露内部 ID
            reason: None,
        }),
        None => Json(StatusResponse {
            status: "not_found".into(),
            hostname: None,
            bitmap: None,
            request_id: None,
            reason: None,
        }),
    }
}

fn hex_decode(s: &str) -> Result<[u8; 32], ()> {
    let bytes = hex::decode(s).map_err(|_| ())?;
    if bytes.len() != 32 {
        return Err(());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn bitmap_to_hex(bm: u64) -> String {
    // 去掉尾随的 0 byte，压缩表示
    let bytes = bm.to_le_bytes();
    let trimmed = &bytes[..bytes.iter().rposition(|&b| b != 0).map(|i| i + 1).unwrap_or(0)];
    hex::encode(trimmed)
}
