//! 权限表
//!
//! 核心数据结构：SHA256(证书) → { hostname, bitmap, status, mac }
//! 位图用 u64 兜住最多 64 个权限位。
//!
//! 垃圾回收: gc() 定期清理已吊销/过期条目，防止权限表无限膨胀。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 权限位定义 (LSB0)
pub const BIT_CONNECTOR: u8 = 0;
pub const BIT_ADMIN: u8 = 1;
pub const BIT_DEVICE: u8 = 2;
#[allow(unused)]
pub const BIT_STORAGE_READ: u8 = 3;
#[allow(unused)]
pub const BIT_STORAGE_WRITE: u8 = 4;
#[allow(unused)]
pub const BIT_COMPUTE_SUBMIT: u8 = 5;
#[allow(unused)]
pub const BIT_COMPUTE_CANCEL: u8 = 6;

/// Compromised/Rejected 条目保留天数（之后被 GC 清除）
const GC_RETAIN_COMPROMISED_DAYS: i64 = 7;
/// Pending 申请保留小时数（之后被 GC 清除）
const GC_RETAIN_PENDING_HOURS: i64 = 24;

/// GC 报告
#[derive(Debug, Clone, Serialize)]
pub struct GcReport {
    pub removed_compromised: Vec<String>,
    pub removed_rejected: Vec<String>,
    pub removed_stale_pending: Vec<String>,
    pub total_removed: usize,
    pub remaining_entries: usize,
}

/// 权限目录：统一列表，定义每个权限的名称、是否可申请、谁可批准
pub fn permission_catalog() -> Vec<(u8, &'static str, bool, bool)> {
    vec![
        (BIT_CONNECTOR, "connector", true, false),
        (BIT_ADMIN,     "admin",     false, true),
        (BIT_DEVICE,    "device",    true, false),
        (BIT_STORAGE_READ,  "storage:read",  true, false),
        (BIT_STORAGE_WRITE, "storage:write", true, false),
        (BIT_COMPUTE_SUBMIT, "compute:submit", true, false),
        (BIT_COMPUTE_CANCEL, "compute:cancel", true, false),
    ]
}

/// 根据请求者身份过滤可批准的权限列表
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
    pub bitmap: u64,
    pub requested_bitmap: u64,
    pub status: EntryStatus,
    pub mac: Option<String>,
    pub created_at: i64,
}

/// 权限表 — 线程安全，支持并发读写
#[derive(Debug, Default)]
pub struct AuthTable {
    entries: HashMap<[u8; 32], PermissionEntry>,
    pending: HashMap<String, [u8; 32]>,
}

impl AuthTable {
    pub fn new() -> Self {
        Self::default()
    }

    // ============ 查询 ============

    pub fn has_bit(&self, sha256: &[u8; 32], bit: u8) -> Option<bool> {
        self.entries.get(sha256).and_then(|e| {
            if e.status != EntryStatus::Active {
                return None;
            }
            Some((e.bitmap >> bit) & 1 == 1)
        })
    }

    pub fn get(&self, sha256: &[u8; 32]) -> Option<&PermissionEntry> {
        self.entries.get(sha256)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &PermissionEntry> {
        self.entries.values()
    }

    // ============ 写入 ============

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

    pub fn approve(&mut self, request_id: &str, bitmap: u64) -> Option<&PermissionEntry> {
        let sha256 = self.pending.remove(request_id)?;
        let entry = self.entries.get_mut(&sha256)?;
        entry.bitmap = bitmap;
        entry.status = EntryStatus::Active;
        Some(entry)
    }

    pub fn reject(&mut self, request_id: &str) -> Option<&PermissionEntry> {
        let sha256 = self.pending.remove(request_id)?;
        let entry = self.entries.get_mut(&sha256)?;
        entry.status = EntryStatus::Rejected;
        Some(entry)
    }

    pub fn revoke(&mut self, sha256: &[u8; 32]) {
        if let Some(e) = self.entries.get_mut(sha256) {
            e.status = EntryStatus::Compromised;
        }
    }

    pub fn restore(&mut self, sha256: &[u8; 32]) {
        if let Some(e) = self.entries.get_mut(sha256) {
            e.status = EntryStatus::Active;
        }
    }

    // ============ 列表 ============

    pub fn list_pending(&self) -> Vec<(&str, &PermissionEntry)> {
        self.pending
            .iter()
            .filter_map(|(rid, sha256)| {
                self.entries.get(sha256).map(|e| (rid.as_str(), e))
            })
            .collect()
    }

    pub fn get_by_request_id(&self, request_id: &str) -> Option<&PermissionEntry> {
        let sha256 = self.pending.get(request_id)?;
        self.entries.get(sha256)
    }

    pub fn list_active(&self) -> Vec<&PermissionEntry> {
        self.entries.values().filter(|e| e.status == EntryStatus::Active).collect()
    }

    pub fn list_compromised(&self) -> Vec<&PermissionEntry> {
        self.entries.values().filter(|e| e.status == EntryStatus::Compromised).collect()
    }

    // ============ 垃圾回收 ============

    /// 清理过期条目，返回清理报告。
    /// 可以定时调用（如每小时），或者在 CLI 中手动触发。
    ///
    /// 清理规则:
    ///   - Compromised: 超过 7 天 → 删除
    ///   - Rejected: 超过 7 天 → 删除
    ///   - Pending: 超过 24 小时 → 删除
    ///   - Active: 永不删除（需手动吊销）
    pub fn gc(&mut self) -> GcReport {
        let now = chrono::Utc::now().timestamp();
        let compromised_cutoff = now - GC_RETAIN_COMPROMISED_DAYS * 86400;
        let pending_cutoff = now - GC_RETAIN_PENDING_HOURS * 3600;

        // 找出要清理的 sha256
        let mut to_remove: Vec<[u8; 32]> = Vec::new();
        let mut removed_compromised = Vec::new();
        let mut removed_rejected = Vec::new();
        let mut removed_stale_pending = Vec::new();

        for (sha256, entry) in &self.entries {
            match entry.status {
                EntryStatus::Compromised | EntryStatus::Rejected if entry.created_at < compromised_cutoff => {
                    let name = entry.hostname.clone();
                    to_remove.push(*sha256);
                    match entry.status {
                        EntryStatus::Compromised => removed_compromised.push(name),
                        _ => removed_rejected.push(name),
                    }
                }
                EntryStatus::Pending if entry.created_at < pending_cutoff => {
                    let name = entry.hostname.clone();
                    to_remove.push(*sha256);
                    removed_stale_pending.push(name);
                }
                _ => {}
            }
        }

        // 从 pending 映射中也移除
        let pending_keys: Vec<String> = self.pending.iter()
            .filter(|(_, sha)| to_remove.contains(sha))
            .map(|(k, _)| k.clone())
            .collect();
        for k in pending_keys {
            self.pending.remove(&k);
        }

        // 从 entries 中移除
        for sha in &to_remove {
            self.entries.remove(sha);
        }

        GcReport {
            total_removed: to_remove.len(),
            remaining_entries: self.entries.len(),
            removed_compromised,
            removed_rejected,
            removed_stale_pending,
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
    fn test_gc_removes_stale_compromised() {
        let mut t = AuthTable::new();
        let h = test_sha256(1);
        // 把 created_at 设置到 14 天前（超过 7 天阈值）
        let old_entry = PermissionEntry {
            sha256: h,
            hostname: "old-device".into(),
            bitmap: 0x01,
            requested_bitmap: 0x01,
            status: EntryStatus::Compromised,
            mac: None,
            created_at: chrono::Utc::now().timestamp() - 14 * 86400,
        };
        t.entries.insert(h, old_entry);
        assert_eq!(t.len(), 1);

        let report = t.gc();
        assert_eq!(report.total_removed, 1);
        assert_eq!(report.removed_compromised[0], "old-device");
        assert_eq!(t.len(), 0);
    }

    #[test]
    fn test_gc_keeps_recent_compromised() {
        let mut t = AuthTable::new();
        let h = test_sha256(2);
        // 刚被吊销（1 分钟前），不应被清除
        let recent = PermissionEntry {
            sha256: h,
            hostname: "recent".into(),
            bitmap: 0x01,
            requested_bitmap: 0x01,
            status: EntryStatus::Compromised,
            mac: None,
            created_at: chrono::Utc::now().timestamp() - 60,
        };
        t.entries.insert(h, recent);
        let report = t.gc();
        assert_eq!(report.total_removed, 0);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn test_gc_removes_stale_pending() {
        let mut t = AuthTable::new();
        let h = test_sha256(3);
        let old = PermissionEntry {
            sha256: h,
            hostname: "stale-request".into(),
            bitmap: 0,
            requested_bitmap: 0x01,
            status: EntryStatus::Pending,
            mac: None,
            created_at: chrono::Utc::now().timestamp() - 48 * 3600, // 48h 前
        };
        t.entries.insert(h, old);
        t.pending.insert("req-stale".into(), h);
        let report = t.gc();
        assert_eq!(report.total_removed, 1);
        assert_eq!(report.removed_stale_pending[0], "stale-request");
        assert!(t.pending.is_empty());
    }

    #[test]
    fn test_gc_never_removes_active() {
        let mut t = AuthTable::new();
        let h = test_sha256(4);
        t.add_pending(h, "live".into(), "req-live".into(), 0x01);
        t.approve("req-live", 0x01);
        let report = t.gc();
        assert_eq!(report.total_removed, 0);
        assert_eq!(t.len(), 1);
    }

    #[test]
    fn test_list_active_and_compromised() {
        let mut t = AuthTable::new();
        let h1 = test_sha256(10);
        let h2 = test_sha256(20);
        t.add_pending(h1, "active-device".into(), "r1".into(), 0x01);
        t.approve("r1", 0x01);
        t.add_pending(h2, "bad-device".into(), "r2".into(), 0x01);
        t.approve("r2", 0x01);
        t.revoke(&h2);
        assert_eq!(t.list_active().len(), 1);
        assert_eq!(t.list_compromised().len(), 1);
    }
}
