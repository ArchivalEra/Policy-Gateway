//! 时间戳事件系统 — EventLog
//!
//! 所有状态变更为不可变事件, append-only。
//! 当前状态 = 重放所有事件(按 timestamp 排序)。
//!
//! 设计目标:
//!   1. 断电不丢: 事件写入 redb 后立即持久化
//!   2. 冲突可解: 同一 sha256 的多个操作, timestamp 最新者胜
//!   3. 同步简单: 交换事件列表, 排序, 重放, 得到一致状态
//!   4. 可审计: 每步都可追踪, 谁在什么时候做了什么

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// 事件类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventKind {
    /// 提交 CSR/pubkey → 进入 pending
    Submitted { hostname: String, requested_bitmap: u64 },
    /// 批准 → active
    Approved { bitmap: u64 },
    /// 拒绝 → rejected
    Rejected,
    /// 确认证书 → confirmed
    Confirmed,
    /// 吊销 → compromised
    Revoked,
    /// 恢复 → active
    Restored,
    /// 权限变更
    PermissionChanged { bitmap: u64 },
}

/// 一个不可变事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// 事件序列号 (单调递增)
    pub seq: u64,
    /// 操作类型
    pub kind: EventKind,
    /// 目标证书 SHA256
    pub sha256: [u8; 32],
    /// Unix 时间戳 (毫秒)
    pub timestamp_ms: i64,
    /// 操作者 (sha256 或 "root")
    pub operator: String,
    /// 可选的设备 ID
    pub device_id: Option<String>,
}

/// 事件日志
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct EventLog {
    events: Vec<Event>,
    next_seq: u64,
}

impl EventLog {
    pub fn new() -> Self { Self::default() }

    /// 追加一个事件
    pub fn push(&mut self, sha256: [u8; 32], kind: EventKind, operator: String, device_id: Option<String>) {
        let event = Event {
            seq: self.next_seq,
            kind,
            sha256,
            timestamp_ms: Utc::now().timestamp_millis(),
            operator,
            device_id,
        };
        self.events.push(event);
        self.next_seq += 1;
    }

    /// 获取某个证书的当前状态（重放到最新）
    pub fn current_state(&self, sha256: &[u8; 32]) -> EventState {
        let mut state = EventState::Unknown;
        for event in &self.events {
            if &event.sha256 == sha256 {
                state = match &event.kind {
                    EventKind::Submitted { .. } => EventState::Pending,
                    EventKind::Approved { .. } => EventState::Active,
                    EventKind::Rejected => EventState::Rejected,
                    EventKind::Confirmed => EventState::Active,
                    EventKind::Revoked => EventState::Compromised,
                    EventKind::Restored => EventState::Active,
                    EventKind::PermissionChanged { .. } => EventState::Active,
                };
            }
        }
        state
    }

    /// 获取某个证书的最终位图
    pub fn current_bitmap(&self, sha256: &[u8; 32]) -> u64 {
        let mut bitmap = 0u64;
        for event in &self.events {
            if &event.sha256 == sha256 {
                match &event.kind {
                    EventKind::Approved { bitmap: b } => bitmap = *b,
                    EventKind::PermissionChanged { bitmap: b } => bitmap = *b,
                    _ => {}
                }
            }
        }
        bitmap
    }

    /// 合并外部事件（同步用）
    pub fn merge(&mut self, external: Vec<Event>) {
        let mut all = self.events.clone();
        all.extend(external);
        // 按 seq 排序去重
        all.sort_by_key(|e| e.seq);
        all.dedup_by_key(|e| e.seq);
        self.events = all;
        self.next_seq = self.events.len() as u64;
    }

    pub fn all_events(&self) -> &[Event] { &self.events }
    pub fn len(&self) -> usize { self.events.len() }
    pub fn is_empty(&self) -> bool { self.events.is_empty() }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventState {
    Unknown,
    Pending,
    Active,
    Rejected,
    Compromised,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_flow() {
        let mut log = EventLog::new();
        let h = [1u8; 32];

        log.push(h, EventKind::Submitted { hostname: "test".into(), requested_bitmap: 1 }, "root".into(), None);
        assert_eq!(log.current_state(&h), EventState::Pending);

        log.push(h, EventKind::Approved { bitmap: 3 }, "admin".into(), None);
        assert_eq!(log.current_state(&h), EventState::Active);
        assert_eq!(log.current_bitmap(&h), 3);
    }

    #[test]
    fn test_revoke_override() {
        let mut log = EventLog::new();
        let h = [2u8; 32];

        log.push(h, EventKind::Submitted { hostname: "d".into(), requested_bitmap: 1 }, "root".into(), None);
        log.push(h, EventKind::Approved { bitmap: 1 }, "admin".into(), None);
        assert_eq!(log.current_state(&h), EventState::Active);

        log.push(h, EventKind::Revoked, "admin".into(), None);
        assert_eq!(log.current_state(&h), EventState::Compromised);

        log.push(h, EventKind::Restored, "root".into(), None);
        assert_eq!(log.current_state(&h), EventState::Active);
    }

    #[test]
    fn test_merge_dedup() {
        let mut log = EventLog::new();
        let h = [3u8; 32];
        log.push(h, EventKind::Submitted { hostname: "t".into(), requested_bitmap: 1 }, "root".into(), None);

        let ext = vec![
            Event { seq: 0, kind: EventKind::Submitted { hostname: "t".into(), requested_bitmap: 1 },
                    sha256: h, timestamp_ms: 1, operator: "root".into(), device_id: None },
            Event { seq: 1, kind: EventKind::Approved { bitmap: 1 },
                    sha256: h, timestamp_ms: 2, operator: "admin".into(), device_id: None },
        ];
        log.merge(ext);
        assert_eq!(log.len(), 2);
        assert_eq!(log.current_state(&h), EventState::Active);
    }
}
