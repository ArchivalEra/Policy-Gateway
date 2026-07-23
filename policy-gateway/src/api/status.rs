//! GET /api/signup/status — 查询申请状态
//!
//! 支持两种查法：
//!   ?id=<request_id>     — 通过申请 ID 查询
//!   ?sha256=<hex>        — 通过证书 SHA256 查询

use axum::extract::{State, Query};
use axum::{Json, response::Html};
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
    /// 建议轮询间隔（秒），MCU 可用此值决定多久查一次
    pub poll_interval: Option<u64>,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StatusQuery>,
) -> Json<StatusResponse> {
    let table = state.auth_table.read().await;

    let entry = if let Some(id) = &q.id {
        table.get_by_request_id(id)
    } else if let Some(hex_str) = &q.sha256 {
        let sha256 = match hex_decode(hex_str) { Ok(h) => h, Err(_) => return Json(StatusResponse { status: "invalid_sha256".into(), hostname: None, bitmap: None, request_id: None, reason: None, poll_interval: None }) };
        table.get(&sha256)
    } else {
        None
    };

    match entry {
        Some(e) => {
            let poll = match e.status {
                crate::auth::EntryStatus::PendingConfirm => Some(15u64),
                crate::auth::EntryStatus::Pending => Some(30u64),
                _ => None,
            };
            Json(StatusResponse {
                status: e.status.to_string(),
                hostname: Some(e.hostname.clone()),
                bitmap: Some(bitmap_to_hex(e.bitmap & crate::auth::valid_bits_mask())),
                request_id: None,
                reason: None,
                poll_interval: poll,
            })
        }
        None => Json(StatusResponse {
            status: "not_found".into(),
            hostname: None,
            bitmap: None,
            request_id: None,
            reason: None,
            poll_interval: None,
        }),
    }
}

/// GET /signup/status — HTML 状态页面（浏览器用）
#[cfg(feature = "frontend")]
pub async fn handle_html(
    State(state): State<Arc<AppState>>,
    Query(q): Query<StatusQuery>,
) -> Html<String> {
    let table = state.auth_table.read().await;
    let (status, hostname) = if let Some(id) = &q.id {
        table.get_by_request_id(id).map(|e| (e.status.to_string(), e.hostname.clone()))
            .unwrap_or(("not_found".into(), "—".into()))
    } else if let Some(hex_str) = &q.sha256 {
        let sha256 = match hex_decode(hex_str) { Ok(h) => h, Err(_) => return Html("<!DOCTYPE html><html><body><h1>sha256 格式无效</h1></body></html>".into()) };
        table.get(&sha256).map(|e| (e.status.to_string(), e.hostname.clone()))
            .unwrap_or(("not_found".into(), "—".into()))
    } else {
        ("missing_query".into(), "—".into())
    };
    Html(format!(r#"<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><title>申请状态</title><style>
body{{font-family:sans-serif;max-width:500px;margin:auto;padding:20px}}
.status{{font-size:24px;padding:20px;border-radius:8px;text-align:center}}
.active{{background:#d4edda;color:#155724}}
.pending{{background:#fff3cd;color:#856404}}
.not_found{{background:#f8d7da;color:#721c24}}
code{{background:#eee;padding:2px 6px}}
</style></head><body>
<h1>申请状态</h1>
<div class="status {status}">{status}</div>
<p>设备: <strong>{hostname}</strong></p>
<p>ID: <code>{id}</code></p>
<p><a href="/signup">返回申请页面</a> · <a href="/manager">管理面板</a></p>
</body></html>"#,
        status = status, hostname = hostname, id = q.id.as_deref().unwrap_or(""),
    ))
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
