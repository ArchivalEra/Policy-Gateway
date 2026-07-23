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
use crate::lang::{t, S};
use crate::config::{hash_token, verify_token};
use std::sync::Arc;

use crate::auth::BIT_CONNECTOR;
use crate::AppState;

/// 启动时读取 MANAGER_TOKEN 哈希
static MANAGER_TOKEN_HASH: Lazy<String> = Lazy::new(|| {
    let cfg = crate::config::Config::load();
    let cfg_hash = cfg.manager_token_hash.clone();
    if !cfg_hash.is_empty() && cfg_hash != hash_token("") {
        return cfg_hash;
    }
    match std::env::var("MANAGER_TOKEN") {
        Ok(t) if !t.is_empty() => hash_token(&t),
        _ => {
            log::error!("MANAGER_TOKEN 未设置！首次使用请运行: policy-gateway init");
            hash_token("")
        }
    }
});

/// Token 验证（哈希比较，防时序攻击）
pub fn check_auth(token: &str) -> bool {
    verify_token(token, &MANAGER_TOKEN_HASH)
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
    #[allow(dead_code)]
    pub reason: Option<String>,
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct ApproveResponse {
    pub status: String,
    pub message: String,
}

/// GET /manager — 返回 HTML 审批页面
#[cfg(feature = "frontend")]
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
<html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>审批面板</title><style>
*{{box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,sans-serif;max-width:960px;margin:0 auto;padding:16px;background:#f5f5f7;color:#1d1d1f}}
.card{{background:#fff;border-radius:12px;padding:16px;margin:12px 0;box-shadow:0 1px 3px#0000001a}}
h1{{font-size:24px;font-weight:600;margin:0 0 16px;display:flex;align-items:center;gap:8px}}
input{{padding:8px 12px;border:1px solid#d1d1d6;border-radius:6px;font-size:14px}}
.btn{{padding:6px 16px;border:none;border-radius:6px;font-size:13px;cursor:pointer}}
.btn-primary{{background:#007aff;color:#fff}}
.btn-approve{{background:#34c759;color:#fff}}
.btn-reject{{background:#ff3b30;color:#fff}}
table{{width:100%;border-collapse:collapse;font-size:14px}}
td,th{{padding:10px 8px;text-align:left;border-bottom:1px solid#e5e5ea}}
th{{font-size:12px;color:#86868b;text-transform:uppercase;letter-spacing:.5px}}
tr:hover{{background:#f5f5f7}}
code{{background:#e8e8ed;padding:2px 6px;border-radius:4px;font-size:12px}}
@media(max-width:600px){{td,th{{display:block}}th{{display:none}}td{{border:none;padding:6px 8px}}td:before{{content:attr(data-label);font-weight:600;display:inline-block;width:80px;font-size:12px;color:#86868b}}}}
</style></head>
<body>
<div class="card"><h1>🔐 审批面板</h1>
<form method="get" style="display:flex;gap:8px;align-items:center;flex-wrap:wrap">
<label style="display:flex;align-items:center;gap:4px">Token: <input type="password" name="token" value="{}" /></label>
<button class="btn btn-primary" type="submit">解锁</button>
</form></div>
{}
<p style="color:#86868b;font-size:12px;margin-top:16px"><a href="/signup" style="color:#007aff">← 返回申请</a></p>
<script>
const TOKEN="{}";
async function approve(id,b){{const r=await fetch('/api/manager/approve',{{method:'POST',headers:{{'Content-Type':'application/json'}},body:JSON.stringify({{request_id:id,action:'approve',bitmap:b,token:TOKEN}})}});alert((await r.json()).message);location.reload()}}
async function reject(id){{const r=await fetch('/api/manager/approve',{{method:'POST',headers:{{'Content-Type':'application/json'}},body:JSON.stringify({{request_id:id,action:'reject',token:TOKEN}})}});alert((await r.json()).message);location.reload()}}
</script>
</body></html>"#,
        safe_token,
        if pending.is_empty() {
            r#"<div class="card"><p style="text-align:center;color:#86868b">✅ 暂无待审批申请</p></div>"#.to_string()
        } else {
            let mut t = r#"<div class="card"><table><tr><th>ID</th><th>主机名</th><th>SHA256</th><th>权限</th><th>状态</th><th>操作</th></tr>"#.to_string();
            for (rid, entry) in &pending {
                let sha256_hex = hex::encode(entry.sha256);
                let safe_hostname = html_escape(&entry.hostname);
                let safe_rid = html_escape(rid);
                t.push_str(&format!(r#"<tr><td data-label="ID"><code>{}</code></td><td data-label="主机名">{}</td><td data-label="SHA256"><code>{}</code></td><td data-label="权限"><code>{}</code></td><td data-label="状态"><code>{}</code></td><td data-label="操作"><button class="btn btn-approve" onclick="approve('{}',1)">批准</button><button class="btn btn-reject" onclick="reject('{}')">驳回</button></td></tr>"#,
                    safe_rid, safe_hostname, &sha256_hex[..12], format!("{:x}", entry.requested_bitmap), entry.status, safe_rid, safe_rid));
            }
            t.push_str(r#"</table></div>"#);
            t
        },
        safe_token
    );
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
                    // SSE 事件推送
                    let _ = state.event_tx.send(serde_json::json!({
                        "type": "approved",
                        "sha256": hex::encode(entry.sha256),
                        "hostname": entry.hostname,
                        "bitmap": req.bitmap.unwrap_or(1),
                    }).to_string());
                    // 记录事件
                    state.event_log.write().await.push(entry.sha256,
                        crate::event_log::EventKind::Approved { bitmap: filtered_bitmap },
                        "admin".into(), None);
                    // 持久化到 redb
                    let sha256_hex = hex::encode(entry.sha256);
                    if let Ok(entry_json) = serde_json::to_string(entry) {
                        let _ = crate::store::put(&sha256_hex, &entry_json).await;
                    }
                    Ok(Json(ApproveResponse {
                        status: "approved".into(), message: format!("已批准 {}", entry.hostname),
                    }))
                }
                None => Err((StatusCode::NOT_FOUND, Json(ApproveResponse {
                    status: "error".into(), message: t(S::ReqNotFound).into(),
                }))),
            }
        }
        "reject" => {
            match table.reject(&req.request_id) {
                Some(entry) => {
                    log::info!("❌ 拒绝: {}", req.request_id);
                    // 记录事件
                    state.event_log.write().await.push(entry.sha256,
                        crate::event_log::EventKind::Rejected,
                        "admin".into(), None);
                    let sha256_hex = hex::encode(entry.sha256);
                    if let Ok(entry_json) = serde_json::to_string(entry) {
                        let _ = crate::store::put(&sha256_hex, &entry_json).await;
                    }
                    Ok(Json(ApproveResponse {
                        status: "rejected".into(), message: "已拒绝".into(),
                    }))
                }
                None => Err((StatusCode::NOT_FOUND, Json(ApproveResponse {
                    status: "error".into(), message: t(S::ReqNotFound).into(),
                }))),
            }
        }
        _ => Err((StatusCode::BAD_REQUEST, Json(ApproveResponse {
            status: "error".into(), message: "未知操作".into(),
        }))),
    }
}

/// JSON 待审批列表（MCU/headless 用）
pub async fn handle_pending_json(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");
    if !check_auth(token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let table = state.auth_table.read().await;
    let pending = table.list_pending();
    let entries: Vec<serde_json::Value> = pending.into_iter().map(|(rid, e)| {
        serde_json::json!({
            "request_id": rid,
            "hostname": e.hostname,
            "sha256": hex::encode(e.sha256),
            "requested_bitmap": format!("{:x}", e.requested_bitmap),
            "hw_platform": e.hw_platform,
            "status": e.status.to_string(),
        })
    }).collect();
    Ok(Json(serde_json::json!({ "count": entries.len(), "entries": entries })))
}
