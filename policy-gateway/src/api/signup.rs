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
    // 1. 解析证书
    let cert = match parse_pem_cert(&req.cert) {
        Some(c) => c,
        None => return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse { error: "证书格式无效".into() }),
        )),
    };

    // 2. 计算 SHA256
    let sha256 = crate::tls::cert_sha256(&cert);
    let sha256_hex = hex::encode(sha256);

    // 3. 查重
    let table = state.auth_table.read().await;
    if table.get(&sha256).is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse { error: "证书已存在，不能重复提交".into() }),
        ));
    }
    drop(table);

    // 4. 生成 request_id，入 pending 队列
    let request_id = Uuid::new_v4().to_string();
    {
        let mut table = state.auth_table.write().await;
        table.add_pending(sha256, req.hostname, request_id.clone());
    }

    log::info!("📝 新证书申请: {} sha256={}", request_id, sha256_hex);

    Ok(Json(SignupResponse {
        request_id,
        status: "pending".into(),
        sha256: sha256_hex,
    }))
}

/// 从 PEM 文本中提取第一个 DER 证书
fn parse_pem_cert(pem_str: &str) -> Option<CertificateDer<'static>> {
    use rustls_pemfile::Item;
    let mut reader = std::io::BufReader::new(pem_str.as_bytes());
    for item in rustls_pemfile::read_all(&mut reader).flatten() {
        if let Item::X509Certificate(der) = item {
            return Some(CertificateDer::from(der.to_vec()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pem_cert_invalid() {
        assert!(parse_pem_cert("not a cert at all").is_none());
    }

    #[test]
    fn test_parse_pem_cert_empty() {
        assert!(parse_pem_cert("").is_none());
    }
}
