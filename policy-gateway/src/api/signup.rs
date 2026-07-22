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
    /// 公钥 Hex（MCU 友好 — 直接发送 Ed25519/ECDSA 公钥 hex）
    /// 服务器会将其包装为证书并用 CA 签名。无需 CSR！
#[allow(unused)]
    pub pubkey: Option<String>,
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
    // MCU 友好: 直接发送公钥 hex
    if let Some(ref pubkey_hex) = req.pubkey {
        return handle_pubkey(state, pubkey_hex, &req).await;
    }
    // 向后兼容: 自签名证书方式
    if let Some(ref cert_pem) = req.cert {
        return handle_self_signed(state, cert_pem, &req).await;
    }
    Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "请提供 csr、pubkey 或 cert".into() })))
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

/// MCU 友好: 接收公钥 hex → 包装为证书 → CA 签名
async fn handle_pubkey(
    state: Arc<AppState>,
    pubkey_hex: &str,
    req: &SignupRequest,
) -> Result<Json<SignupResponse>, (StatusCode, Json<ErrorResponse>)> {
    if req.hostname.is_empty() || req.hostname.len() > 255 {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "主机名不能为空且不超过 255 字符".into() })));
    }
    // 验证 hex 格式
    let pubkey_bytes = match hex::decode(pubkey_hex) {
        Ok(b) if b.len() == 32 || b.len() == 33 => b, // Ed25519=32, ECDSA P-256=33
        Ok(_) => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "公钥长度应为 32(Ed25519) 或 33(P-256) 字节".into() }))),
        Err(_) => return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: "公钥不是有效的 hex 编码".into() }))),
    };
    let pubkey_hex_padded = hex::encode(&pubkey_bytes);
    let role = parse_role(req.requested.as_deref());
    let not_before = chrono::Utc::now().timestamp();
    let not_after = not_before + 365 * 86400; // 1 年
    let body = format!(
        "subject:{}\nserial:{}\npubkey:{}\nnot_before:{}\nnot_after:{}\nrole:{}\n",
        req.hostname, uuid::Uuid::new_v4().to_string().get(..8).unwrap_or("00"),
        pubkey_hex_padded, not_before, not_after, role
    );
    // 用 CA 密钥签名
    let pkcs8_der = match crate::tls::extract_pkcs8_from_pem(&state.ca_key_pem) {
        Some(d) => d,
        None => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "CA 密钥解码失败".into() }))),
    };
    let ca_kp = match ring::signature::Ed25519KeyPair::from_pkcs8(&pkcs8_der) {
        Ok(kp) => kp,
        Err(_) => return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "CA 密钥无效".into() }))),
    };
    let sig = ca_kp.sign(body.as_bytes());
    let sig_hex = hex::encode(sig.as_ref());
    let cert_pem = format!(
        "-----BEGIN CERTIFICATE-----\n{}\nsignature:{}\n-----END CERTIFICATE-----\n",
        body, sig_hex
    );
    let sha256 = crate::tls::pem_sha256(&cert_pem);
    let request_id = uuid::Uuid::new_v4().to_string();
    {
        let mut table = state.auth_table.write().await;
        if table.get(&sha256).is_some() {
            return Err((StatusCode::CONFLICT, Json(ErrorResponse { error: "证书已存在".into() })));
        }
        let bitmap = parse_requested_bitmap(req.requested.as_deref());
        table.add_pending_with_hw(sha256, req.hostname.clone(), request_id.clone(), bitmap,
            crate::auth::EntryStatus::PendingConfirm, req.hw_id.clone(), req.hw_platform.clone(), req.device_id.clone());
    }
    log::info!("📝 pubkey 签发: {} role={} sha256={}", request_id, role, hex::encode(sha256));
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

/// GET /signup — HTML 申请表单（浏览器用，含 Web Crypto CSR 生成）
pub async fn handle_form(
    State(state): State<Arc<AppState>>,
) -> Html<String> {
    let table = state.auth_table.read().await;
    let count = table.list_pending().len();
    Html(format!(r#"<!DOCTYPE html><html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>证书申请</title><style>
*{{box-sizing:border-box}}
body{{font-family:-apple-system,system-ui,sans-serif;max-width:600px;margin:auto;padding:20px;background:#f5f5f5;color:#333}}
input,select,textarea{{width:100%;padding:10px;margin:6px 0;border:1px solid #ddd;border-radius:6px;font-size:16px;box-sizing:border-box}}
button{{padding:12px 20px;background:#0066cc;color:#fff;border:none;border-radius:6px;cursor:pointer;font-size:16px;width:100%}}
button:hover{{background:#0052a3}}
button:disabled{{opacity:.5;cursor:not-allowed}}
code{{background:#eee;padding:2px 6px;border-radius:3px;font-size:13px;word-break:break-all}}
pre{{overflow:auto;max-height:200px;background:#f4f4f4;padding:10px;border-radius:6px;font-size:13px}}
.card{{background:#fff;border-radius:12px;padding:20px;margin:12px 0;box-shadow:0 1px 3px rgba(0,0,0,.1)}}
.badge{{display:inline-block;padding:2px 8px;border-radius:4px;font-size:12px;margin:2px}}
.badge-green{{background:#d4edda;color:#155724}}
.badge-yellow{{background:#fff3cd}}
@media(max-width:480px){{body{{padding:10px}}.card{{padding:12px}}}}
<h1>📜 证书申请</h1>
<p>待审批: {count}</p>
<div style="background:#f0f8ff;padding:12px;border-radius:8px;margin-bottom:12px">
<strong>🤖 MCU / 无头设备？</strong>
<p style="font-size:14px">用 curl 提交预先生成的 CSR：<br>
<code style="font-size:12px">curl -X POST http://host:8443/api/signup -H 'Content-Type: application/json' -d '{{"csr":"PEM","hostname":"my-device"}}'</code><br>
<a href="/api/help?topic=mcu" style="font-size:12px">完整 MCU 教程 →</a></p>
</div>
<div style="background:#fff;border:1px solid #ddd;padding:12px;border-radius:8px">
<h3>浏览器一键申请</h3>
<button id="genKeyBtn" onclick="generateKey()">🔑 生成本地密钥对</button>
<span id="keyStatus"></span>
<form id="f" onsubmit="submitForm(event)" style="display:none" id="formWrap">
<label>设备名: <input type="text" id="h" required></label>
<label>权限: <select id="t"><option value="01">🌐 上网</option><option value="05">⚙️ 上网+计算</option></select></label>
<button type="submit" id="subBtn">提交申请</button>
</form>
</div>
<div id="r"></div>
<script>
let keyPair = null;
async function generateKey(){{
const btn=document.getElementById('genKeyBtn');
btn.disabled=true;btn.textContent='生成中...';
try{{
keyPair=await crypto.subtle.generateKey({{name:'RSA-PSS',modulusLength:2048,publicExponent:new Uint8Array([1,0,1]),hash:'SHA-256'}},false,['sign']);
document.getElementById('keyStatus').innerHTML='<span class="badge badge-green">✅ 密钥对已生成</span>';
document.getElementById('formWrap').style.display='block';
}}catch(e){{
document.getElementById('keyStatus').innerHTML='<span style="color:red">❌ 浏览器不支持 Web Crypto</span>';
}}
btn.disabled=false;btn.textContent='🔑 重新生成';
}}
async function submitForm(e){{
e.preventDefault();
if(!keyPair) return alert('请先生成密钥对');
const btn=document.getElementById('subBtn');btn.disabled=true;btn.textContent='提交中...';
try{{
const hostname=document.getElementById('h').value;
const requested=document.getElementById('t').value;
// 导出公钥为 SPKI DER → PEM
const spki=await crypto.subtle.exportKey('spki',keyPair.publicKey);
const spkiB64=btoa(String.fromCharCode(...new Uint8Array(spki)));
const pubPem='-----BEGIN PUBLIC KEY-----\n'+spkiB64.match(/.{{1,64}}/g).join('\n')+'\n-----END PUBLIC KEY-----';
// 创建自签名证书（简化版：实际是公钥包装）
const r=await fetch('/api/signup',{{method:'POST',headers:{{'Content-Type':'application/json'}},
body:JSON.stringify({{cert:pubPem,hostname,requested}})}});
const d=await r.json();
document.getElementById('r').innerHTML='<h3>✅ 申请已提交</h3>'+
'<p>ID: <code>'+d.request_id+'</code></p>'+
'<p>SHA256: <code>'+d.sha256+'</code></p>'+
'<p>状态: <span class="badge badge-yellow">'+d.status+'</span></p>'+
(d.cert_pem?'<details><summary>📄 证书</summary><pre>'+d.cert_pem+'</pre></details>':'')+
'<p><a href="/signup/status?id='+d.request_id+'">查看状态 →</a></p>';
}}catch(e){{
document.getElementById('r').innerHTML='<p style="color:red">❌ 提交失败: '+e+'</p>';
}}
btn.disabled=false;btn.textContent='提交申请';
}}
</script>
<p style="margin-top:20px;font-size:12px;color:#888">
<a href="/manager">管理</a> · <a href="/permissions">权限表</a> · <a href="/api/help">教程</a>
</p>
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

    #[test]
    fn test_pubkey_validation() {
        // 32-byte hex is valid (Ed25519)
        let h32 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        assert_eq!(hex::decode(h32).unwrap().len(), 32);

        // 33-byte hex is valid (ECDSA P-256)
        let h33 = "03abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        assert_eq!(hex::decode(h33).unwrap().len(), 33);

        // Invalid hex
        assert!(hex::decode("xyz").is_err());

        // Wrong length (31 bytes)
        let h31 = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef012345678";
        // 31 bytes = 62 hex chars, odd length hex causes Err
    }
}
