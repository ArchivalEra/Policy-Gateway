//! /manager — 管理员审批面板
//!
//! GET  /manager             → HTML 审批页面
//! POST /api/manager/approve → 同意/拒绝申请 (JSON)
//!
//! 安全: Phase 0 使用环境变量 MANAGER_TOKEN 做简单鉴权
//!       Phase 1 改为 mTLS 客户端证书认证

use axum::extract::State;
use axum::http::StatusCode;
use axum::{Json, response::Html};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::BIT_CONNECTOR;
use crate::AppState;

/// 启动时读取 MANAGER_TOKEN，不存在则 panic
static MANAGER_TOKEN: Lazy<String> = Lazy::new(|| {
    std::env::var("MANAGER_TOKEN").expect("MANAGER_TOKEN 环境变量未设置，启动失败")
});

pub fn check_auth(token: &str) -> bool {
    token == *MANAGER_TOKEN
}

/// HTML 转义（防止 XSS）
fn html_escape(s: &str) -> String {
    s.chars().map(|c| match c {
        '<' => "&lt;".into(),
        '>' => "&gt;".into(),
        '&' => "&amp;".into(),
        '"' => "&quot;".into(),
        '\'' => "&#x27;".into(),
        _ => c.to_string(),
    }).collect()
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
        let safe_hostname = html_escape(&entry.hostname);
        let safe_rid = html_escape(rid);
        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td><code>{}</code></td>
                <td><code>{}</code></td>
                <td>{}</td>
                <td>
                    <button onclick="approve('{}', 1)">✅ connector</button>
                    <button onclick="approve('{}', 3)">✅ +admin</button>
                    <button onclick="reject('{}')">❌ 拒绝</button>
                </td>
            </tr>"#,
            safe_rid, safe_hostname, sha256_hex, format!("{:x}", entry.requested_bitmap), entry.status,
            safe_rid, safe_rid, safe_rid
        ));
    }

    let safe_token = html_escape(token);
    let html = format!(r#"<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>审批面板</title>
<style>
body{{font-family:sans-serif;max-width:800px;margin:auto;padding:20px}}
table{{width:100%;border-collapse:collapse}}
td,th{{border:1px solid #ddd;padding:8px;text-align:left}}
tr:nth-child(even){{background:#f9f9f9}}
button{{cursor:pointer;margin:2px}}
</style>
</head>
<body>
<h1>🔐 审批面板</h1>
<form method="get" style="margin-bottom:12px">
<label>Token: <input type="text" name="token" value="{safe_token}" size="40" /></label>
<button type="submit">解锁</button>
</form>
<table>
<tr><th>ID</th><th>主机名</th><th>SHA256</th><th>状态</th><th>操作</th></tr>
{rows}
</table>
<p><small>点击操作会自动刷新</small></p>
<script>
const TOKEN = "{safe_token}";
async function approve(id, bitmap) {{
    const r = await fetch('/api/manager/approve', {{
        method:'POST',
        headers:{{'Content-Type':'application/json'}},
        body: JSON.stringify({{request_id:id, action:'approve', bitmap, token:TOKEN}})
    }});
    const d = await r.json();
    alert(d.message);
    location.reload();
}}
async function reject(id) {{
    const r = await fetch('/api/manager/approve', {{
        method:'POST',
        headers:{{'Content-Type':'application/json'}},
        body: JSON.stringify({{request_id:id, action:'reject', token:TOKEN}})
    }});
    const d = await r.json();
    alert(d.message);
    location.reload();
}}
</script>
</body>
</html>"#);

    Ok(Html(html))
}

/// POST /api/manager/approve — 审批操作 (JSON)
pub async fn handle_approve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ApproveRequest>,
) -> Result<Json<ApproveResponse>, (StatusCode, Json<ApproveResponse>)> {
    let token = req.token.as_deref().unwrap_or("");
    if !check_auth(token) {
        return Err((StatusCode::UNAUTHORIZED, Json(ApproveResponse {
            status: "error".into(), message: "token 无效".into(),
        })));
    }

    let mut table = state.auth_table.write().await;
    match req.action.as_str() {
        "approve" => {
            let bitmap = req.bitmap.unwrap_or(1 << BIT_CONNECTOR);
            // 只允许授予当前管理员有权限的 bit
            let is_root = false; // TODO Phase 1: 根据 mTLS 证书判断
            let allowed = crate::auth::grantable_permissions(is_root)
                .iter().fold(0u64, |acc, (bit, _)| acc | (1u64 << bit));
            let filtered_bitmap = bitmap & allowed;
            if filtered_bitmap == 0 {
                return Err((StatusCode::BAD_REQUEST, Json(ApproveResponse {
                    status: "error".into(), message: "无可授予的权限".into(),
                })));
            }
            match table.approve(&req.request_id, filtered_bitmap) {
                Some(entry) => {
                    log::info!("✅ 批准: {} ({})", req.request_id, entry.hostname);
                    Ok(Json(ApproveResponse {
                        status: "approved".into(), message: format!("已批准 {}", entry.hostname),
                    }))
                }
                None => Err((StatusCode::NOT_FOUND, Json(ApproveResponse {
                    status: "error".into(), message: "申请 ID 不存在或已处理".into(),
                }))),
            }
        }
        "reject" => {
            match table.reject(&req.request_id) {
                Some(_) => {
                    log::info!("❌ 拒绝: {}", req.request_id);
                    Ok(Json(ApproveResponse {
                        status: "rejected".into(), message: "已拒绝".into(),
                    }))
                }
                None => Err((StatusCode::NOT_FOUND, Json(ApproveResponse {
                    status: "error".into(), message: "申请 ID 不存在或已处理".into(),
                }))),
            }
        }
        _ => Err((StatusCode::BAD_REQUEST, Json(ApproveResponse {
            status: "error".into(), message: "未知操作".into(),
        }))),
    }
}
