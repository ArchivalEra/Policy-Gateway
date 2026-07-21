#![allow(dead_code)]
//! Recovery — 根证书恢复系统
//!
//! 三种模式:
//!   1. 短恢复码（8位，SHA256 存储）
//!   2. 证书加密恢复（AES-256-GCM，密钥派生自证书，无明文存储）
//!   3. 关闭恢复
//!
//! 恢复页面 /recover 仅在无根证书时可见。

use ring::digest::{digest, SHA256};
use ring::rand::{SecureRandom, SystemRandom};

/// 恢复码字符集（排除易混淆字符 0OIl1）
const CODE_CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghjkmnpqrstuvwxyz23456789#@$!";
const CODE_LENGTH: usize = 8;

/// 恢复配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum RecoveryConfig {
    /// 关闭恢复
    Disabled,
    /// 短恢复码模式（存储 SHA256）
    ShortCode { sha256: [u8; 32] },
    /// 证书加密模式（AES-256-GCM 加密 blob）
    CertEncrypted {
        encrypted_blob: Vec<u8>,
        nonce: Vec<u8>,
        tag: Vec<u8>,
    },
}

impl RecoveryConfig {
    /// 是否启用恢复
    pub fn is_enabled(&self) -> bool {
        !matches!(self, RecoveryConfig::Disabled)
    }
}

/// 生成 8 位短恢复码 + SHA256
pub fn generate_short_code() -> (String, [u8; 32]) {
    let rng = SystemRandom::new();
    let mut code = String::with_capacity(CODE_LENGTH);
    let mut buf = [0u8; 1];

    for _ in 0..CODE_LENGTH {
        loop {
            // 密码学安全随机数
            if rng.fill(&mut buf).is_err() { continue; }
            let idx = buf[0] as usize % CODE_CHARS.len();
            code.push(CODE_CHARS[idx] as char);
            break;
        }
    }

    let hash = digest(&SHA256, code.as_bytes());
    let mut sha256 = [0u8; 32];
    sha256.copy_from_slice(hash.as_ref());
    (code, sha256)
}

/// 验证短恢复码（比对 SHA256）
pub fn verify_short_code(code: &str, expected_sha256: &[u8; 32]) -> bool {
    let hash = digest(&SHA256, code.as_bytes());
    let mut sha256 = [0u8; 32];
    sha256.copy_from_slice(hash.as_ref());
    sha256 == expected_sha256.as_ref()
}

/// 生成证书加密恢复数据
///
/// 密钥派生: root_cert_pem 的前 32 字节 → AES-256 密钥
/// 加密数据: SHA256(root_cert_pem)
pub fn generate_cert_encrypted(cert_pem: &str) -> Option<RecoveryConfig> {
    let cert_bytes = cert_pem.as_bytes();
    if cert_bytes.len() < 32 {
        return None; // 证书太短
    }

    // 密钥 = 证书前 32 字节
    let key = &cert_bytes[..32];
    let data = digest(&SHA256, cert_bytes);
    let data_bytes = data.as_ref();

    // 使用 ring 的 aead 进行 AES-256-GCM 加密
    use ring::aead::{AES_256_GCM, Nonce, UnboundKey, LessSafeKey, Aad};
    use ring::rand::SystemRandom;

    let rng = SystemRandom::new();
    let mut nonce_bytes = [0u8; 12]; // GCM nonce = 12 bytes
    rng.fill(&mut nonce_bytes).ok()?;

    let unbound_key = UnboundKey::new(&AES_256_GCM, key).ok()?;
    let key = LessSafeKey::new(unbound_key);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = data_bytes.to_vec();
    // GCM 加密，返回 tag 放在 in_out 末尾
    key.seal_in_place_append_tag(nonce, Aad::empty(), &mut in_out).ok()?;

    // in_out 现在包含密文 + 16 字节 tag
    let tag_start = in_out.len() - 16;
    let encrypted = in_out[..tag_start].to_vec();
    let tag = in_out[tag_start..].to_vec();

    Some(RecoveryConfig::CertEncrypted {
        encrypted_blob: encrypted,
        nonce: nonce_bytes.to_vec(),
        tag,
    })
}

/// 验证证书加密恢复
///
/// 传入 root_cert_pem，尝试解密并比对 SHA256
pub fn verify_cert_encrypted(cert_pem: &str, config: &RecoveryConfig) -> bool {
    let (encrypted_blob, nonce_bytes, tag) = match config {
        RecoveryConfig::CertEncrypted { encrypted_blob, nonce, tag } => {
            (encrypted_blob, nonce, tag)
        }
        _ => return false,
    };

    let cert_bytes = cert_pem.as_bytes();
    if cert_bytes.len() < 32 {
        return false;
    }

    let key = &cert_bytes[..32];
    let expected_sha256 = digest(&SHA256, cert_bytes);

    use ring::aead::{AES_256_GCM, Nonce, UnboundKey, LessSafeKey, Aad};

    let unbound_key = match UnboundKey::new(&AES_256_GCM, key) {
        Ok(k) => k,
        Err(_) => return false,
    };
    let key_lsk = LessSafeKey::new(unbound_key);
    let nonce = match <[u8; 12]>::try_from(&nonce_bytes[..12]) {
        Ok(n) => Nonce::assume_unique_for_key(n),
        Err(_) => return false,
    };

    let mut in_out = encrypted_blob.clone();
    in_out.extend_from_slice(tag);

    match key_lsk.open_in_place(nonce, Aad::empty(), &mut in_out) {
        Ok(decrypted) => {
            decrypted == expected_sha256.as_ref()
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_code_generation() {
        let (code, sha256) = generate_short_code();
        assert_eq!(code.len(), 8);
        assert!(code.chars().all(|c| c.is_ascii_graphic()));
        assert!(verify_short_code(&code, &sha256));
    }

    #[test]
    fn test_short_code_rejection() {
        let (_, sha256) = generate_short_code();
        assert!(!verify_short_code("wrongcode", &sha256));
    }

    #[test]
    fn test_cert_encrypted_roundtrip() {
        let cert = "-----BEGIN CERTIFICATE-----\nMIIB...test cert...\n-----END CERTIFICATE-----\n";
        let padded = format!("{:0>64}", cert); // pad to 64+ bytes
        let config = generate_cert_encrypted(&padded).unwrap();
        assert!(matches!(config, RecoveryConfig::CertEncrypted { .. }));
        assert!(verify_cert_encrypted(&padded, &config));
    }

    #[test]
    fn test_cert_encrypted_wrong_cert() {
        let cert = "-----BEGIN CERTIFICATE-----\nREAL CERT DATA...\n-----END CERTIFICATE-----\n";
        let padded = format!("{:0>64}", cert);
        let config = generate_cert_encrypted(&padded).unwrap();

        let wrong_cert = "-----BEGIN CERTIFICATE-----\nFAKE CERT...\n-----END CERTIFICATE-----\n";
        let padded_wrong = format!("{:0>64}", wrong_cert);
        assert!(!verify_cert_encrypted(&padded_wrong, &config));
    }
}
