//! POST /api/signup — 提交证书申请

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct SignupRequest {
    /// PEM 格式的自签名证书
    pub cert: String,
    /// 设备主机名（任意 Unicode）
    pub hostname: String,
    /// 申请的权限位图（hex），如 "05" = connector+device
    /// 不传或传空则默认只申请 connector
    pub requested: Option<String>,
}

#[derive(Serialize)]
pub struct SignupResponse {
    pub request_id: String,
    pub status: String,
    pub sha256: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn handle(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SignupRequest>,
) -> Result<Json<SignupResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 1. 解析并验证证书 DER 格式
    let cert = match parse_and_validate_cert(&req.cert) {
        Some(c) => c,
        None => return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "证书格式无效，请提交合法的 X.509 PEM 证书".into() }),
        )),
    };

    // 2. 计算 SHA256
    let sha256 = crate::tls::cert_sha256(&cert);
    let sha256_hex = hex::encode(sha256);

    // 3. 验证 hostname 长度和字符集
    if req.hostname.is_empty() || req.hostname.len() > 255 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "主机名不能为空且不超过 255 字符".into() }),
        ));
    }
    // 只允许可打印字符，防止 XSS
    // 51418bb8 UnicodeFf0c53ea62d27edd63a752365b577b26
    if req.hostname.chars().any(|c| c.is_control()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "主机名包含不允许的字符".into() }),
        ));
    }

    // 4. 在写锁内原子执行：查重 → 插入（防 TOCTOU）
    let request_id = Uuid::new_v4().to_string();
    {
        let mut table = state.auth_table.write().await;
        if table.get(&sha256).is_some() {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse { error: "证书已存在，不能重复提交".into() }),
            ));
        }
        let req_bitmap = parse_requested_bitmap(req.requested.as_deref());
        table.add_pending(sha256, req.hostname.clone(), request_id.clone(), req_bitmap);
    }

    log::info!("📝 新证书申请: {} hostname={} sha256={}", request_id, req.hostname, sha256_hex);

    Ok(Json(SignupResponse {
        request_id,
        status: "pending".into(),
        sha256: sha256_hex,
    }))
}

/// 解析请求的权限位图（hex 字符串），默认返回 connector
fn parse_requested_bitmap(s: Option<&str>) -> u64 {
    let raw = match s {
        Some(h) if !h.is_empty() => {
            u64::from_str_radix(h, 16).unwrap_or(1u64 << crate::auth::BIT_CONNECTOR)
        }
        _ => 1u64 << crate::auth::BIT_CONNECTOR,
    };
    // 过滤掉不可申请的权限（如 admin）
    let mask = crate::auth::permission_catalog()
        .iter().filter(|(_, _, r, _)| *r)
        .fold(0u64, |acc, (bit, _, _, _)| acc | (1u64 << bit));
    raw & mask
}

/// 解析 PEM 并验证其为合法的 X.509 证书（至少 DER 能解析）
fn parse_and_validate_cert(pem_str: &str) -> Option<CertificateDer<'static>> {
    use rustls_pemfile::Item;

    let mut reader = std::io::BufReader::new(pem_str.as_bytes());
    let der_bytes = rustls_pemfile::read_all(&mut reader)
        .filter_map(|r| r.ok())
        .find_map(|item| {
            if let Item::X509Certificate(der) = item {
                Some(der.to_vec())
            } else {
                None
            }
        })?;

    // 用 x509-parser 验证 DER 是合法的 X.509 证书
    use x509_parser::prelude::*;
    if parse_x509_certificate(&der_bytes).is_err() {
        return None;
    }

    Some(CertificateDer::from(der_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_invalid() {
        assert!(parse_and_validate_cert("not a cert").is_none());
    }

    #[test]
    fn test_parse_empty() {
        assert!(parse_and_validate_cert("").is_none());
    }

    #[test]
    fn test_parse_random_bytes() {
        // 随机字节应该被 x509-parser 拒绝
        let junk = vec![0x00, 0x01, 0x02, 0x03];
        let pem = format!("-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----", base64::encode(&junk));
        assert!(parse_and_validate_cert(&pem).is_none());
    }
}
