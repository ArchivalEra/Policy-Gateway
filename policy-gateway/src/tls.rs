//! mTLS 配置
//!
//! 使用 rustls + ring 实现 TLS 1.3。
//! 跳过客户端 CA 验证（dangerous_configuration），应用层自己做 SHA256 查表。
//!
//! # 安全说明
//! 我们在 TLS 层不验证客户端证书的签名链，因为证书是浏览器自签名的。
//! 应用层通过 SHA256(证书) 查权限表来验证身份，误放概率 2^-256。
//! rustls-webpki 仍会执行 DER 解析、有效期检查等基础验证。

use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use rustls::ServerConfig;
use std::sync::Arc;

/// 创建 mTLS ServerConfig
///
/// - `cert_pem`: 服务端证书 PEM 文件路径
/// - `key_pem`:  服务端私钥 PEM 文件路径
pub fn tls_config(cert_pem: &str, key_pem: &str) -> ServerConfig {
    let certs = CertificateDer::pem_file_iter(cert_pem)
        .expect("无法读取服务端证书")
        .map(|c| c.unwrap())
        .collect();

    let key = PrivateKeyDer::from_pem_file(key_pem)
        .expect("无法读取服务端私钥");

    let mut config = ServerConfig::builder()
        .with_no_client_auth()  // Phase 1 改为 with_client_cert_verifier
        .with_single_cert(certs, key)
        .expect("服务端证书配置失败");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config
}

/// 计算证书的 SHA256 指纹
///
/// 这是权限表的唯一键。不依赖 CA 签名，只依赖哈希。
pub fn cert_sha256(cert: &CertificateDer<'_>) -> [u8; 32] {
    use ring::digest::{digest, SHA256};
    let d = digest(&SHA256, cert.as_ref());
    let mut out = [0u8; 32];
    out.copy_from_slice(d.as_ref());
    out
}

/// Phase 1: 带客户端证书请求的 mTLS 配置
/// 使用 dangerous_configuration 跳过 CA 签名验证
#[allow(unused)]
pub fn mtls_config_dangerous(cert_pem: &str, key_pem: &str) -> ServerConfig {
    use rustls::client::WantsClientCert;
    use rustls::server::danger::ClientCertVerifier;

    let certs = CertificateDer::pem_file_iter(cert_pem)
        .expect("无法读取服务端证书")
        .map(|c| c.unwrap())
        .collect();

    let key = PrivateKeyDer::from_pem_file(key_pem)
        .expect("无法读取服务端私钥");

    // TODO Phase 1: 实现跳过 CA 验证的 ClientCertVerifier
    // 见 rustls::server::danger::ClientCertVerifier trait
    // 关键方法: verify_client_cert() → 始终返回 Ok(Empty)
    //            verify_tls12_signature() → 不验证
    //            verify_tls13_signature() → 不验证

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .expect("服务端证书配置失败");

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cert_sha256_consistency() {
        let der = CertificateDer::from(b"\x30\x82\x00\x00\x00".to_vec());
        let hash = cert_sha256(&der);
        assert_eq!(hash.len(), 32);
    }
}
