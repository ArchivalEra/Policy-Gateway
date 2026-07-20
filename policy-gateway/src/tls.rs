//! mTLS 配置 —— rustls dangerous_configuration
//! 跳过 CA 签名校验，应用层自己做 SHA256 查表

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use std::sync::Arc;

pub fn tls_config(cert_pem: &str, key_pem: &str) -> ServerConfig {
    // TODO: 加载服务端证书 + 启用客户端证书请求
    // TODO: dangerous_configuration: skip_client_cert_verification
    unimplemented!()
}

pub fn cert_sha256(cert: &CertificateDer) -> [u8; 32] {
    use ring::digest::{digest, SHA256};
    let d = digest(&SHA256, cert);
    d.as_ref().try_into().unwrap()
}
