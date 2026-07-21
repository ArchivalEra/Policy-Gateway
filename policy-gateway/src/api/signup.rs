//! POST /api/signup — 提交证书申请（CA 签发版）
//!
//! 两阶段确认:
//!   1. 提交 CSR → CA 签名 → 存 pending_confirm → 返回 .crt
//!   2. 客户端确认 → 标记 active
//!
//! 硬件 ID 可选字段，帮助设备绑定。

use axum::extract::State;
use axum::{Json, response::Html};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;
use crate::auth::EntryStatus;

#[derive(Deserialize)]
pub struct SignupRequest {
    /// PEM 格式的 CSR（证书签名请求）
    pub csr: Option<String>,
    /// PEM 格式的自签名证书（向后兼容，推荐用 CSR）
    pub cert: Option<String>,
    /// 设备主机名（任意 Unicode）
    pub hostname: String,
    /// 申请的权限位图（hex），默认 "01"
    pub requested: Option<String>,
    /// 硬件 ID (SHA256 of platform-specific ID)
    pub hw_id: Option<String>,
    /// 硬件平台: browser, ios, android, windows, linux, esp32, stm32
    pub hw_platform: Option<String>,
    /// 持久设备 ID（客户端生成的随机 UUID，保护隐私）
    pub device_id: Option<String>,
}

#[derive(Serialize)]
pub struct SignupResponse {
    pub request_id: String,
    pub status: String,
    pub sha256: String,
    pub cert_pem: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<SignupResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 优先使用 CSR 方式
    if let Some(ref csr_pem) = req.csr {
        return handle_csr(state, csr_pem, &req).await;
    }
    // 向后兼容: 自签名证书方式
    if let Some(ref cert_pem) = req.cert {
        return handle_self_signed(state, cert_pem, &req).await;
    }
    Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "请提供 csr 或 cert".into() })))
}

/// CA 签发模式: 接收 CSR → 签名 → pending_confirm
async fn handle_csr(
    state: Arc<AppState>,
    csr_pem: &str,
    req: &SignupRequest,
) -> Result<Json<SignupResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 1. 验证 hostname
    if req.hostname.is_empty() || req.hostname.len() > 255 {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "主机名不能为空且不超过 255 字符".into() })));
    }
    if req.hostname.chars().any(|c| c.is_control()) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "主机名包含不允许的字符".into() })));
    }

    // 2. 解析 CSR
    let csr_info = match crate::tls::parse_csr(csr_pem) {
        Some(info) => info,
        None => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "CSR 格式无效".into() }))),
    };

    // 3. 确定角色
    let role = parse_role(req.requested.as_deref());

    // 4. CA 签名
    let cert_pem = match crate::tls::sign_csr(&csr_info, &state.ca_key_pem, &role) {
        Some(cert) => cert,
        None => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "证书签名失败".into() }))),
    };

    // 5. 计算 SHA256
    let sha256 = crate::tls::pem_sha256(&cert_pem);

    // 6. 查重
    let request_id = Uuid::new_v4().to_string();
    {
        let mut table = state.auth_table.write().await;
        if table.get(&sha256).is_some() {
            return Err((StatusCode::CONFLICT, Json(ErrorResponse { error: "证书已存在".into() })));
        }
        // 存入 pending_confirm（跳过审批，CA 签发即可信）
        let bitmap = parse_requested_bitmap(req.requested.as_deref());
        table.add_pending_with_hw(sha256, req.hostname.clone(), request_id.clone(), bitmap, EntryStatus::PendingConfirm, req.hw_id.clone(), req.hw_platform.clone(), req.device_id.clone());
    }

    log::info!("📝 CA 签发: {} role={} sha256={}", request_id, role, hex::encode(sha256));

    Ok(Json(SignupResponse {
        request_id,
        status: "pending_confirm".into(),
        sha256: hex::encode(sha256),
        cert_pem: Some(cert_pem),
    }))
}

/// 向后兼容: 自签名证书模式
async fn handle_self_signed(
    state: Arc<AppState>,
    cert_pem: &str,
    req: &SignupRequest,
) -> Result<Json<SignupResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 原始的解析 + 查重流程
    let cert = match parse_and_validate_cert(cert_pem) {
        Some(c) => c,
        None => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "证书格式无效".into() }))),
    };
    let sha256 = crate::tls::cert_sha256(&cert);
    let request_id = Uuid::new_v4().to_string();
    {
        let mut table = state.auth_table.write().await;
        if table.get(&sha256).is_some() {
            return Err((StatusCode::CONFLICT, Json(ErrorResponse { error: "证书已存在".into() })));
        }
        let bitmap = parse_requested_bitmap(req.requested.as_deref());
        table.add_pending(sha256, req.hostname.clone(), request_id.clone(), bitmap);
    }
    Ok(Json(SignupResponse { request_id, status: "pending".into(), sha256: hex::encode(sha256), cert_pem: None }))
}

fn parse_role(requested: Option<&str>) -> &'static str {
    // 用 parse_requested_bitmap 过滤后的位图判断角色
    // 确保 admin/root 不可通过申请获得
    let bitmap = parse_requested_bitmap(requested);
    if bitmap & (1 << crate::auth::BIT_ADMIN) != 0 {
        "connector"  // admin bit 被过滤后不可能为真，降级到 connector
    } else if bitmap & (1 << crate::auth::BIT_DEVICE) != 0 {
        "device"
    } else {
        "connector"
    }
}

fn parse_requested_bitmap(s: Option<&str>) -> u64 {
    let raw = match s {
        Some(h) if !h.is_empty() => u64::from_str_radix(h, 16).unwrap_or(1u64 << crate::auth::BIT_CONNECTOR),
        _ => 1u64 << crate::auth::BIT_CONNECTOR,
    };
    let mask = crate::auth::permission_catalog()
        .iter().filter(|(_, _, r, _)| *r)
        .fold(0u64, |acc, (bit, _, _, _)| acc | (1u64 << bit));
    raw & mask
}

fn parse_and_validate_cert(pem_str: &str) -> Option<rustls::pki_types::CertificateDer<'static>> {
    use rustls_pemfile::Item;
    use x509_parser::prelude::*;
    let mut reader = std::io::BufReader::new(pem_str.as_bytes());
    let der_bytes = rustls_pemfile::read_all(&mut reader)
        .filter_map(|r| r.ok())
        .find_map(|item| if let Item::X509Certificate(der) = item { Some(der.to_vec()) } else { None })?;
    if parse_x509_certificate(&der_bytes).is_err() { return None; }
    Some(rustls::pki_types::CertificateDer::from(der_bytes))
}

/// GET /signup — HTML 申请表单（浏览器用）
pub async fn handle_form(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let table = state.auth_table.read().await;
    let count = table.list_pending().len();
    Html(format!(r#"<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><title>证书申请</title><style>
body{{font-family:sans-serif;max-width:600px;margin:auto;padding:20px}}
input,select,textarea{{width:100%;padding:8px;margin:6px 0}}
button{{padding:10px 20px;background:#06c;color:#fff;border:none}}
code{{background:#eee;padding:2px 6px}}
</style></head><body>
<h1>📜 证书申请</h1>
<p>待审批: {count}</p>
<form id="f" onsubmit="s(event)">
<label>设备名: <input type="text" id="h" required></label>
<label>模板: <select id="t"><option value="01">🌐 上网</option><option value="05">⚙️ 上网+计算</option></select></label>
<label>CSR: <textarea id="c" rows="5" placeholder="-----BEGIN CERTIFICATE REQUEST-----"></textarea></label>
<button type="submit">提交</button></form><div id="r"></div>
<script>
async function s(e){{e.preventDefault();
const r=await fetch('/api/signup',{{method:'POST',headers:{{'Content-Type':'application/json'}},
body:JSON.stringify({{csr:document.getElementById('c').value,hostname:document.getElementById('h').value,requested:document.getElementById('t').value}})}});
const d=await r.json();
document.getElementById('r').innerHTML=d.cert_pem?'<h3>✅ 提交成功</h3><p>ID: <code>'+d.request_id+'</code></p><details><summary>证书</summary><pre>'+d.cert_pem+'</pre></details><p><a href=\"/api/signup/status?id='+d.request_id+'\">状态</a></p>':'<h3>📋 提交成功</h3><p>ID: <code>'+d.request_id+'</code> 待审批</p>';
}}</script>
<p><a href="/manager">管理</a> · <a href="/permissions">权限表</a></p>
</body></html>"#))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_parse_role() {
        // admin/root bits are filtered by parse_requested_bitmap
        assert_eq!(parse_role(Some("01")), "connector");
        assert_eq!(parse_role(Some("05")), "device");
        assert_eq!(parse_role(Some("03")), "connector");  // admin filtered -> connector
        assert_eq!(parse_role(Some("FF")), "device");  // "FF" has device bit   // root filtered -> connector
    }
    #[test]
    fn test_parse_invalid() { assert!(parse_and_validate_cert("bad").is_none()); }
}
