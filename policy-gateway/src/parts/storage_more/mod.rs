#![allow(dead_code)]
//! storage-more — 可扩展存储模块
//!
//! 在核心 redb (bit 0-2) 之外提供自定义存储后端:
//!   - 自定义路径 (USB/SD/tmpfs/闪存)
//!   - S3 兼容对象存储 (R2 / MinIO / Oracle S3) [stub]
//!   - 多文件分片 [stub]
//!
//! 注册 bit 3+。模块不可用时核心自动锁住这些 bit。

use crate::parts::{CoreState, GatewayModule, ModuleDeclaration};
use axum::Router;

pub const CLAIM_BITS: &[u8] = &[3, 4];

/// storage-more 模块结构
pub struct StorageMore {
    name: &'static str,
    version: u32,
    /// 存储后端类型: "dir" / "s3" / "tmpfs"
    backend: String,
    /// 存储路径 (dir 后端使用)
    path: Option<String>,
}

impl StorageMore {
    pub fn new(path: Option<&str>, backend: Option<&str>) -> Self {
        Self {
            name: "storage-more",
            version: 1,
            backend: backend.unwrap_or("dir").to_string(),
            path: path.map(|s| s.to_string()),
        }
    }
}

impl GatewayModule for StorageMore {
    fn name(&self) -> &'static str {
        self.name
    }

    fn declaration(&self) -> ModuleDeclaration {
        ModuleDeclaration {
            name: self.name,
            version: self.version,
            claim_bits: CLAIM_BITS.to_vec(),
            min_core_version: "0.3.3",
        }
    }

    fn required_permission(&self) -> Option<u8> {
        None // storage-more 不需要特定权限才能访问
    }

    fn mount(&self, _state: CoreState) -> Router {
        Router::new()
        // TODO Phase 3.4+: 注册存储管理 API
        // GET  /api/storage/status — 当前后端状态
        // POST /api/storage/sync   — 手动同步
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_declaration() {
        let sm = StorageMore::new(None, None);
        let decl = sm.declaration();
        assert_eq!(decl.name, "storage-more");
        assert_eq!(decl.version, 1);
        assert_eq!(decl.claim_bits, vec![3, 4]);
        assert_eq!(decl.min_core_version, "0.3.3");
    }
}
