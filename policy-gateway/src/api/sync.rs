//! /sync — 路由器和 Worker 之间的双向同步
//!
//! 架构: Worker 是仲裁者，路由器是缓存。
//!
//! 同步协议:
//!   1. 路由器上线后发 POST /sync/push { events: [{id, type, sha256, ...}] }
//!   2. Worker 接收事件，按顺序重放到事件日志
//!   3. Worker 返回 POST /sync/push 响应 { accepted: [...], rejected: [...] }
//!   4. 路由器拉取 GET /sync/pull → { state: [...], version: N }
//!   5. 路由器用 Worker 的 state 覆盖本地权限表
//!
//! 冲突规则:
//!   - Worker 始终是权威。路由器离线期间产生的状态在同步时被 Worker 覆盖。
//!   - 事件是幂等的: revoke > approve > reject（revoke 永远赢）
//!   - 时间戳仅用于顺序，不用于仲裁

use crate::auth::PermissionEntry;
use serde::{Deserialize, Serialize};

/// 同步路由器的入站事件请求
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SyncPushRequest {
    /// 路由器产生的事件列表
    pub events: Vec<SyncEvent>,
    /// 路由器当前知道的 Worker 版本号（用于增量同步）
    pub known_version: u64,
}

/// 同步响应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SyncPushResponse {
    pub accepted: Vec<String>,  // 接受的事件 ID
    pub rejected: Vec<RejectedEvent>,  // 拒绝的事件
    pub new_version: u64,       // Worker 当前版本号
    pub full_state: Vec<SyncEntry>,  // 全量权限表（路由器用这个覆盖本地）
}

/// 被拒绝的事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RejectedEvent {
    pub id: String,
    pub reason: String,
}

/// 单个同步事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    /// 设备申请证书
    Signup { id: String, sha256: String, hostname: String, requested_bitmap: u64, created_at: i64 },
    /// 管理员批准
    Approve { id: String, request_id: String, sha256: String, bitmap: u64, by: String, timestamp: i64 },
    /// 管理员拒绝
    Reject { id: String, request_id: String, sha256: String, by: String, timestamp: i64 },
    /// 吊销
    Revoke { id: String, sha256: String, by: String, timestamp: i64 },
    /// 恢复
    Restore { id: String, sha256: String, by: String, timestamp: i64 },
    /// 心跳（证明设备在线）
    Heartbeat { id: String, sha256: String, mac: String, timestamp: i64 },
}

/// 权限表条目（用于同步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SyncEntry {
    pub sha256: String,
    pub hostname: String,
    pub bitmap: u64,
    pub status: String,  // "active" | "pending" | "compromised" | "rejected"
    pub mac: Option<String>,
    pub created_at: i64,
    pub last_seen: Option<i64>,
}

impl SyncEntry {
    #[allow(dead_code)]
pub fn from_entry(entry: &PermissionEntry) -> Self {
        SyncEntry {
            sha256: hex::encode(entry.sha256),
            hostname: entry.hostname.clone(),
            bitmap: entry.bitmap,
            status: entry.status.to_string(),
            mac: entry.mac.clone(),
            created_at: entry.created_at,
            last_seen: entry.last_seen,
        }
    }
}

/// 事件冲突处理
#[allow(dead_code)]
pub fn resolve_conflict(existing: Option<&SyncEntry>, event: &SyncEvent) -> bool {
    match event {
        // Revoke 永远赢：一旦吊销，只能由 Revoke 或 Restore 改变
        SyncEvent::Revoke { sha256: _, .. } => {
            if let Some(e) = existing {
                if e.status == "compromised" {
                    return false; // 已吊销，重复事件
                }
            }
            true
        }
        // Restore 可以逆转 Revoke
        SyncEvent::Restore { sha256: _, .. } => {
            if let Some(e) = existing {
                if e.status == "active" {
                    return false; // 已经是 active
                }
            }
            true
        }
        // Approve 不能覆盖 Revoke
        SyncEvent::Approve { sha256: _, .. } => {
            if let Some(e) = existing {
                if e.status == "compromised" {
                    return false; // 已吊销，拒绝批准
                }
            }
            true
        }
        // Reject 可以覆盖 Approve
        SyncEvent::Reject { sha256: _, .. } => true,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(status: &str) -> SyncEntry {
        SyncEntry {
            sha256: "ab".into(), hostname: "t".into(), bitmap: 1,
            status: status.into(), mac: None, created_at: 0, last_seen: None,
        }
    }

    #[test]
    fn test_revoke_wins() {
        let entry = make_entry("active");
        let event = SyncEvent::Revoke {
            id: "e1".into(), sha256: "ab".into(), by: "admin".into(), timestamp: 1,
        };
        assert!(resolve_conflict(Some(&entry), &event));
    }

    #[test]
    fn test_approve_cannot_override_revoke() {
        let entry = make_entry("compromised");
        let event = SyncEvent::Approve {
            id: "e2".into(), request_id: "r1".into(), sha256: "ab".into(),
            bitmap: 5, by: "admin".into(), timestamp: 2,
        };
        assert!(!resolve_conflict(Some(&entry), &event));
    }

    #[test]
    fn test_restore_works() {
        let entry = make_entry("compromised");
        let event = SyncEvent::Restore {
            id: "e3".into(), sha256: "ab".into(), by: "root".into(), timestamp: 3,
        };
        assert!(resolve_conflict(Some(&entry), &event));
    }
}
