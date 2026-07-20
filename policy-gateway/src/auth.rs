//! 权限表 —— SHA256(证书) → 位图

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PermissionEntry {
    pub sha256: [u8; 32],
    pub hostname: String,
    pub bitmap: u64,       // 变长位图，目前用 u64 兜住 64 个权限
    pub status: EntryStatus,
    pub mac: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EntryStatus {
    Active,
    Pending,
    Compromised,
    Rejected,
}

pub struct AuthTable {
    entries: HashMap<[u8; 32], PermissionEntry>,
}

impl AuthTable {
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    pub fn has_bit(&self, sha256: &[u8; 32], bit: u8) -> Option<bool> {
        self.entries.get(sha256).map(|e| (e.bitmap >> bit) & 1 == 1)
    }
}
