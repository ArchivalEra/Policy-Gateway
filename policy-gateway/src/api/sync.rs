//! /sync — 路由器和 Worker 之间的双向同步
//!
//! Phase 0 实现：序列化/反序列化权限表，为双向同步打好基础。
//! 两端共用同一套数据格式，通过 HTTP 交换。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::auth::{AuthTable, PermissionEntry, EntryStatus};

/// 同步数据包
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncPacket {
    /// 数据版本号（单调递增，用于 last-write-wins）
    pub version: u64,
    /// 全量权限表
    pub entries: Vec<SyncEntry>,
    /// 待审批队列
    pub pending: Vec<PendingSync>,
    /// 发送方时间戳
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncEntry {
    pub sha256: String,       // hex
    pub hostname: String,
    pub bitmap: u64,
    pub status: String,       // "active" | "pending" | "compromised" | "rejected"
    pub mac: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingSync {
    pub request_id: String,
    pub sha256: String,
}

impl SyncPacket {
    /// 从 AuthTable 构建同步包
    pub fn from_table(version: u64, table: &AuthTable) -> Self {
        // 这里需要访问 AuthTable 内部数据
        // 由于 AuthTable 是 HashMap，需要导出方法
        todo!("Phase 2: 完整实现同步序列化")
    }

    /// 合并到本地的 AuthTable（last-write-wins）
    pub fn merge_into(self, table: &mut AuthTable) -> u64 {
        // 比较 version，只接受更新的数据
        todo!("Phase 2: 完整实现同步合并")
    }
}

/// 序列化权限表条目为 SyncEntry (供 Worker 使用)
pub fn entry_to_sync_entry(entry: &PermissionEntry) -> SyncEntry {
    SyncEntry {
        sha256: hex::encode(entry.sha256),
        hostname: entry.hostname.clone(),
        bitmap: entry.bitmap,
        status: entry.status.to_string(),
        mac: entry.mac.clone(),
        created_at: entry.created_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_entry_roundtrip() {
        let entry = PermissionEntry {
            sha256: [1u8; 32],
            hostname: "test".into(),
            bitmap: 0x01,
            requested_bitmap: 0x01,
            status: EntryStatus::Active,
            mac: None,
            created_at: 1234567890,
        };
        let sync = entry_to_sync_entry(&entry);
        assert_eq!(sync.sha256, hex::encode([1u8; 32]));
        assert_eq!(sync.hostname, "test");
        assert_eq!(sync.bitmap, 0x01);
    }
}
