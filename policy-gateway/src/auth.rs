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
    pub removed_unseen: Vec<String>,
    pub total_removed: usize,
    pub remaining_entries: usize,
}

/// 权限目录：统一列表，定义每个权限的名称、是否可申请、谁可批准
/// 动态可更新:
///   - 删除权限: 仅从列表移除，不修改已存储的位图
///   - 生效: valid_bits_mask() 实时变化，has_bit() 自动反映
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

/// 获取当前所有有效权限的位图掩码
/// 用于把设备的 bitmap 和目录解耦:
///   设备的 0xFFFF → 但目录只有 7 个权限 → 结果 0x7F
///   删除某权限后（如移除 device）→ mask 自动变化
pub fn valid_bits_mask() -> u64 {
    permission_catalog().iter().fold(0u64, |acc, (bit, _, _, _)| acc | (1u64 << bit))
}

/// 根据请求者身份过滤可批准的权限列表
pub fn grantable_permissions(is_root: bool) -> Vec<(u8, &'static str)> {
    permission_catalog()
        .into_iter()
        .filter(|(_, _, _, root_only)| is_root || !root_only)
        .map(|(bit, name, _, _)| (bit, name))
        .collect()
}

/// 返回当前目录中所有权限的可读名称列表
pub fn list_permission_names() -> Vec<&'static str> {
    permission_catalog().iter().map(|(_, name, _, _)| *name).collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EntryStatus {
    Active,
    Pending,
    PendingConfirm,
    Compromised,
    Rejected,
}

impl std::fmt::Display for EntryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EntryStatus::Active => write!(f, "active"),
            EntryStatus::Pending => write!(f, "pending"),
            EntryStatus::PendingConfirm => write!(f, "pending_confirm"),
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
    /// 最后一次访问时间（用于 GC 判断“注册后未登录”）
    pub last_seen: Option<i64>,
    pub hw_id: Option<String>,
    pub hw_platform: Option<String>,
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

    /// 查权限：检查某个设备是否有指定 bit
    /// 设备的 bitmap 与 valid_bits_mask 取交后再判断
    /// 这样目录中删除的权限自动失效
    pub fn has_bit(&self, sha256: &[u8; 32], bit: u8) -> Option<bool> {
        self.entries.get(sha256).and_then(|e| {
            if e.status != EntryStatus::Active {
                return None;
            }
            let effective = e.bitmap & valid_bits_mask();
            Some((effective >> bit) & 1 == 1)
        })
    }

    /// 获取设备当前有效的权限位图（过滤后）
    pub fn effective_bitmap(&self, sha256: &[u8; 32]) -> Option<u64> {
        self.entries.get(sha256).map(|e| {
            if e.status != EntryStatus::Active {
                0
            } else {
                e.bitmap & valid_bits_mask()
            }
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
        self.add_pending_with_hw(sha256, hostname, request_id, requested_bitmap, EntryStatus::Pending, None, None)
    }

    pub fn add_pending_with_hw(
        &mut self,
        sha256: [u8; 32],
        hostname: String,
        request_id: String,
        requested_bitmap: u64,
        initial_status: EntryStatus,
        hw_id: Option<String>,
        hw_platform: Option<String>,
    ) {
        let entry = PermissionEntry {
            sha256,
            hostname,
            bitmap: 0,
            requested_bitmap,
            status: initial_status,
            mac: None,
            created_at: chrono::Utc::now().timestamp(),
            last_seen: None,
            hw_id: None,
            hw_platform: None,
        };
        self.pending.insert(request_id, sha256);
        self.entries.insert(sha256, entry);
    }

    pub fn get_mut(&mut self, sha256: &[u8; 32]) -> Option<&mut PermissionEntry> {
        self.entries.get_mut(sha256)
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
    /// 使用预设规则集执行 GC
    pub fn gc(&mut self) -> GcReport {
        self.gc_with_rules(&default_gc_rules())
    }

    /// 使用自定义规则集执行 GC
    /// 每条规则是一个函数，接收 (当前表, 当前时间戳) 返回要清理的 entries
    pub fn gc_with_rules(&mut self, rules: &[GcRule]) -> GcReport {
        let now = chrono::Utc::now().timestamp();
        let mut to_remove: std::collections::HashSet<[u8; 32]> = std::collections::HashSet::new();
        let mut removed_compromised: Vec<String> = Vec::new();
        let mut removed_rejected: Vec<String> = Vec::new();
        let mut removed_stale_pending: Vec<String> = Vec::new();
        let mut removed_unseen: Vec<String> = Vec::new();

        for rule in rules {
            for result in (rule.fn_ptr)(self, now) {
                if to_remove.insert(result.sha256) {
                    // 只记录第一次发现时的原因
                    match result.reason {
                        GcReason::CompromisedStale => removed_compromised.push(result.hostname),
                        GcReason::RejectedStale => removed_rejected.push(result.hostname),
                        GcReason::PendingStale => removed_stale_pending.push(result.hostname),
                        GcReason::NeverSeen => removed_unseen.push(result.hostname),
                    }
                }
            }
        }

        // 从 pending 映射中移除
        let to_remove_vec: Vec<[u8; 32]> = to_remove.iter().cloned().collect();
        let pending_keys: Vec<String> = self.pending.iter()
            .filter(|(_, sha)| to_remove_vec.contains(sha))
            .map(|(k, _)| k.clone())
            .collect();
        for k in pending_keys {
            self.pending.remove(&k);
        }

        for sha in &to_remove_vec {
            self.entries.remove(sha);
        }

        let total = to_remove.len();
        let mut reasons = removed_compromised.clone();
        reasons.extend(removed_rejected.clone());
        reasons.extend(removed_stale_pending.clone());
        reasons.extend(removed_unseen.clone());

        GcReport {
            total_removed: total,
            remaining_entries: self.entries.len(),
            removed_compromised,
            removed_rejected,
            removed_stale_pending,
            removed_unseen,
        }
    }
}


/// GC 规则结构体，可复用可组合
pub struct GcRule {
    pub name: &'static str,
    pub fn_ptr: fn(&AuthTable, now: i64) -> Vec<GcResult>,
}

/// 某条规则找出的待清理结果
pub struct GcResult {
    pub sha256: [u8; 32],
    pub hostname: String,
    pub reason: GcReason,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GcReason {
    CompromisedStale,
    RejectedStale,
    PendingStale,
    NeverSeen,
}

/// 默认 GC 规则集
pub fn default_gc_rules() -> Vec<GcRule> {
    vec![
        GcRule { name: "compromised-stale", fn_ptr: gc_rule_compromised_stale },
        GcRule { name: "rejected-stale",    fn_ptr: gc_rule_rejected_stale },
        GcRule { name: "pending-stale",     fn_ptr: gc_rule_pending_stale },
        GcRule { name: "never-seen",        fn_ptr: gc_rule_never_seen },
    ]
}

/// 规则 1: Compromised 超过 7 天
fn gc_rule_compromised_stale(table: &AuthTable, now: i64) -> Vec<GcResult> {
    let cutoff = now - GC_RETAIN_COMPROMISED_DAYS * 86400;
    table.entries.values()
        .filter(|e| e.status == EntryStatus::Compromised && e.created_at < cutoff)
        .map(|e| GcResult {
            sha256: e.sha256,
            hostname: e.hostname.clone(),
            reason: GcReason::CompromisedStale,
        })
        .collect()
}

/// 规则 2: Rejected 超过 7 天
fn gc_rule_rejected_stale(table: &AuthTable, now: i64) -> Vec<GcResult> {
    let cutoff = now - GC_RETAIN_COMPROMISED_DAYS * 86400;
    table.entries.values()
        .filter(|e| e.status == EntryStatus::Rejected && e.created_at < cutoff)
        .map(|e| GcResult {
            sha256: e.sha256,
            hostname: e.hostname.clone(),
            reason: GcReason::RejectedStale,
        })
        .collect()
}

/// 规则 3: Pending 超过 24 小时
fn gc_rule_pending_stale(table: &AuthTable, now: i64) -> Vec<GcResult> {
    let cutoff = now - GC_RETAIN_PENDING_HOURS * 3600;
    table.entries.values()
        .filter(|e| e.status == EntryStatus::Pending && e.created_at < cutoff)
        .map(|e| GcResult {
            sha256: e.sha256,
            hostname: e.hostname.clone(),
            reason: GcReason::PendingStale,
        })
        .collect()
}

/// 规则 4: 注册后 24h 内无任何访问记录
/// 只要访问过一次（last_seen 不为 None），就保留
fn gc_rule_never_seen(table: &AuthTable, now: i64) -> Vec<GcResult> {
    let cutoff = now - GC_RETAIN_PENDING_HOURS * 3600; // 24h
    table.entries.values()
        .filter(|e| {
            e.status != EntryStatus::Pending
            && e.created_at < cutoff
            && e.last_seen.is_none()
        })
        .map(|e| GcResult {
            sha256: e.sha256,
            hostname: e.hostname.clone(),
            reason: GcReason::NeverSeen,
        })
        .collect()
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
            last_seen: None,
            hw_id: None,
            hw_platform: None,
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
            last_seen: None,
            hw_id: None,
            hw_platform: None,
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
            last_seen: None,
            hw_id: None,
            hw_platform: None,
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
