//! /manager — 管理员审批面板
//!
//! GET  /manager             → HTML 审批页面
//! POST /api/manager/approve → 同意/拒绝申请
//!
//! 安全: Phase 0 使用环境变量 MANAGER_TOKEN 做简单鉴权
//!       Phase 1 改为 mTLS 客户端证书认证

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, response::Html};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::BIT_CONNECTOR;
use crate::AppState;

/// 从环境变量读取管理令牌
fn check_auth(token: &str) -> bool {
    let expected = std::env::var("MANAGER_TOKEN").unwrap_or_else(|_| "dev-token".into());
    token == expected
}

#[derive(Deserialize)]
pub struct ApproveRequest {
    pub request_id: String,
    pub action: String,      // "approve" | "reject"
    pub bitmap: Option<u64>, // 批准时设置的权限位图
    pub reason: Option<String>,
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct ApproveResponse {
    pub status: String,
    pub message: String,
}

/// GET /manager — 返回 HTML 审批页面
pub async fn handle_page(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");
    if !check_auth(token) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let table = state.auth_table.read().await;
    let pending = table.list_pending();

    let mut rows = String::new();
    for (rid, entry) in &pending {
        let sha256_hex = hex::encode(entry.sha256);
        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td><code>{}</code></td>
                <td>{}</td>
                <td>
                    <form action="/api/manager/approve?token={token}" method="post" style="display:inline">
                        <input type="hidden" name="request_id" value="{}" />
                        <input type="hidden" name="action" value="approve" />
                        <label>bitmap: <input type="text" name="bitmap" value="01" size="4" /></label>
                        <button type="submit">✅ 同意</button>
                    </form>
                    <form action="/api/manager/approve?token={token}" method="post" style="display:inline">
                        <input type="hidden" name="request_id" value="{}" />
                        <input type="hidden" name="action" value="reject" />
                        <button type="submit">❌ 拒绝</button>
                    </form>
                </td>
            </tr>"#,
            rid, entry.hostname, sha256_hex, entry.status, rid, rid
        ));
    }

    let html = format!(r#"<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>审批面板</title>
<style>body{{font-family:sans-serif;max-width:800px;margin:auto;padding:20px}}
table{{width:100%;border-collapse:collapse}}
td,th{{border:1px solid #ddd;padding:8px;text-align:left}}
tr:nth-child(even){{background:#f9f9f9}}
button{{cursor:pointer}}
.approve{{color:green}} .reject{{color:red}}</style>
</head>
<body>
<h1>🔐 审批面板</h1>
<form method="get">
<label>Token: <input type="text" name="token" size="40" /></label>
<button type="submit">解锁</button>
</form>
<table>
<tr><th>ID</th><th>主机名</th><th>SHA256</th><th>状态</th><th>操作</th></tr>
{rows}
</table>
<p><small>bitmap: 01=connector, 03=connector+admin, 05=connector+device</small></p>
</body>
</html>"#);

    Ok(Html(html))
}

/// POST /api/manager/approve — 审批操作
pub async fn handle_approve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApproveRequest>,
) -> Result<Json<ApproveResponse>, (StatusCode, Json<ApproveResponse>)> {
    let token = req.token.as_deref().unwrap_or("");
    if !check_auth(token) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ApproveResponse {
                status: "error".into(),
                message: "token 无效".into(),
            }),
        ));
    }

    let mut table = state.auth_table.write().await;

    match req.action.as_str() {
        "approve" => {
            let bitmap = req.bitmap.unwrap_or(1 << BIT_CONNECTOR);
            match table.approve(&req.request_id, bitmap) {
                Some(entry) => {
                    log::info!("✅ 批准: {} ({} bitmap={:x})", req.request_id, entry.hostname, bitmap);
                    Ok(Json(ApproveResponse {
                        status: "approved".into(),
                        message: format!("已批准 {}，位图={:x}", entry.hostname, bitmap),
                    }))
                }
                None => Err((
                    StatusCode::NOT_FOUND,
                    Json(ApproveResponse {
                        status: "error".into(),
                        message: "申请 ID 不存在或已处理".into(),
                    }),
                )),
            }
        }
        "reject" => {
            match table.reject(&req.request_id) {
                Some(_) => {
                    log::info!("❌ 拒绝: {}", req.request_id);
                    Ok(Json(ApproveResponse {
                        status: "rejected".into(),
                        message: "已拒绝".into(),
                    }))
                }
                None => Err((
                    StatusCode::NOT_FOUND,
                    Json(ApproveResponse {
                        status: "error".into(),
                        message: "申请 ID 不存在或已处理".into(),
                    }),
                )),
            }
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ApproveResponse {
                status: "error".into(),
                message: "未知操作，可用: approve / reject".into(),
            }),
        )),
    }
}
