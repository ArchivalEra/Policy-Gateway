//! 权限表
//!
//! 核心数据结构：SHA256(证书) → { hostname, bitmap, status, mac }
//! 位图用 u64 兜住最多 64 个权限位。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 权限位定义 (LSB0)
pub const BIT_CONNECTOR: u8 = 0;    // 上网
pub const BIT_ADMIN: u8 = 1;        // 管理员
pub const BIT_DEVICE: u8 = 2;       // 算力节点
#[allow(unused)]
pub const BIT_STORAGE_READ: u8 = 3;
#[allow(unused)]
pub const BIT_STORAGE_WRITE: u8 = 4;
#[allow(unused)]
pub const BIT_COMPUTE_SUBMIT: u8 = 5;
#[allow(unused)]
pub const BIT_COMPUTE_CANCEL: u8 = 6;

/// 权限目录：统一列表，定义每个权限的名称、是否可申请、谁可批准
/// 返回 Vec<(bit, 名称, 可申请?, 仅根管理员批准?)>
pub fn permission_catalog() -> Vec<(u8, &'static str, bool, bool)> {
    vec![
        (BIT_CONNECTOR, "connector", true, false),      // 上网 — 可申请，管理员可批准
        (BIT_ADMIN,     "admin",     false, true),      // 管理员 — 不可申请，仅根可批准
        (BIT_DEVICE,    "device",    true, false),      // 算力节点 — 可申请，管理员可批准
        (BIT_STORAGE_READ,  "storage:read",  true, false),
        (BIT_STORAGE_WRITE, "storage:write", true, false),
        (BIT_COMPUTE_SUBMIT, "compute:submit", true, false),
        (BIT_COMPUTE_CANCEL, "compute:cancel", true, false),
    ]
}

/// 根据请求者身份过滤可批准的权限列表
/// is_root: 请求者是否为根管理员
pub fn grantable_permissions(is_root: bool) -> Vec<(u8, &'static str)> {
    permission_catalog()
        .into_iter()
        .filter(|(_, _, _, root_only)| is_root || !root_only)
        .map(|(bit, name, _, _)| (bit, name))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntryStatus {
    Active,
    Pending,
    Compromised,
    Rejected,
}

impl std::fmt::Display for EntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryStatus::Active => write!(f, "active"),
            EntryStatus::Pending => write!(f, "pending"),
            EntryStatus::Compromised => write!(f, "compromised"),
            EntryStatus::Rejected => write!(f, "rejected"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEntry {
    pub sha256: [u8; 32],
    pub hostname: String,
    /// 位图，每 bit 代表一个权限
    pub bitmap: u64,
    /// 申请的权限（审批时参考），admin 可批准子集
    pub requested_bitmap: u64,
    pub status: EntryStatus,
    pub mac: Option<String>,
    pub created_at: i64,
}

/// 权限表 — 线程安全，支持并发读写
#[derive(Debug, Default)]
pub struct AuthTable {
    /// SHA256 → 权限条目
    entries: HashMap<[u8; 32], PermissionEntry>,
    /// request_id → SHA256 (用于 pending 状态查询)
    pending: HashMap<String, [u8; 32]>,
}

impl AuthTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// 查权限：检查某个设备是否有指定 bit
    /// 只有 Active 状态的条目才返回 Some，其余返回 None
    pub fn has_bit(&self, sha256: &[u8; 32], bit: u8) -> Option<bool> {
        self.entries.get(sha256).and_then(|e| {
            if e.status != EntryStatus::Active {
                return None;
            }
            Some((e.bitmap >> bit) & 1 == 1)
        })
    }

    /// 获取条目
    pub fn get(&self, sha256: &[u8; 32]) -> Option<&PermissionEntry> {
        self.entries.get(sha256)
    }

    /// 添加 pending 申请（调用方确保不重复）
    pub fn add_pending(
        &mut self,
        sha256: [u8; 32],
        hostname: String,
        request_id: String,
        requested_bitmap: u64,
    ) {
        let entry = PermissionEntry {
            sha256,
            hostname,
            bitmap: 0,
            requested_bitmap,
            status: EntryStatus::Pending,
            mac: None,
            created_at: chrono::Utc::now().timestamp(),
        };
        self.pending.insert(request_id, sha256);
        self.entries.insert(sha256, entry);
    }

    /// 审批通过：设置位图并激活，从 pending 映射移除
    pub fn approve(&mut self, request_id: &str, bitmap: u64) -> Option<&PermissionEntry> {
        let sha256 = self.pending.remove(request_id)?;  // 移除 pending，防止重复审批
        let entry = self.entries.get_mut(&sha256)?;
        entry.bitmap = bitmap;
        entry.status = EntryStatus::Active;
        Some(entry)
    }

    /// 拒绝并移除 pending
    pub fn reject(&mut self, request_id: &str) -> Option<&PermissionEntry> {
        let sha256 = self.pending.remove(request_id)?;
        let entry = self.entries.get_mut(&sha256)?;
        entry.status = EntryStatus::Rejected;
        Some(entry)
    }

    /// 获取 pending 申请列表
    pub fn list_pending(&self) -> Vec<(&str, &PermissionEntry)> {
        self.pending
            .iter()
            .filter_map(|(rid, sha256)| {
                self.entries.get(sha256).map(|e| (rid.as_str(), e))
            })
            .collect()
    }

    /// 通过 request_id 查找条目
    pub fn get_by_request_id(&self, request_id: &str) -> Option<&PermissionEntry> {
        let sha256 = self.pending.get(request_id)?;
        self.entries.get(sha256)
    }

    /// 吊销（标记 compromised）
    pub fn revoke(&mut self, sha256: &[u8; 32]) {
        if let Some(e) = self.entries.get_mut(sha256) {
            e.status = EntryStatus::Compromised;
        }
    }

    /// 恢复（改回 Active）
    pub fn restore(&mut self, sha256: &[u8; 32]) {
        if let Some(e) = self.entries.get_mut(sha256) {
            e.status = EntryStatus::Active;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_sha256(val: u8) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = val;
        h
    }

    #[test]
    fn test_add_and_approve() {
        let mut t = AuthTable::new();
        let h = test_sha256(1);
        t.add_pending(h, "test-pc".into(), "req-1".into(), 0x01);

        assert_eq!(t.get(&h).unwrap().status, EntryStatus::Pending);
        assert!(t.has_bit(&h, BIT_CONNECTOR).is_none()); // pending 不暴露 bit

        t.approve("req-1", 1 << BIT_CONNECTOR);
        assert_eq!(t.get(&h).unwrap().status, EntryStatus::Active);
        assert_eq!(t.has_bit(&h, BIT_CONNECTOR), Some(true));
    }

    #[test]
    fn test_approve_idempotent() {
        // approve 后再次 approve 同一 request_id 应返回 None
        let mut t = AuthTable::new();
        let h = test_sha256(5);
        t.add_pending(h, "test".into(), "req-5".into(), 0x01);
        assert!(t.approve("req-5", 0x01).is_some());
        assert!(t.approve("req-5", 0x03).is_none()); // 已从 pending 移除
    }

    #[test]
    fn test_revoke_and_restore() {
        let mut t = AuthTable::new();
        let h = test_sha256(2);
        t.add_pending(h, "nas".into(), "req-2".into(), 0x01);
        t.approve("req-2", 1 << BIT_CONNECTOR);

        t.revoke(&h);
        assert_eq!(t.get(&h).unwrap().status, EntryStatus::Compromised);
        assert!(t.has_bit(&h, BIT_CONNECTOR).is_none()); // compromised 不暴露 bit

        t.restore(&h);
        assert_eq!(t.get(&h).unwrap().status, EntryStatus::Active);
        assert_eq!(t.has_bit(&h, BIT_CONNECTOR), Some(true));
    }
}
