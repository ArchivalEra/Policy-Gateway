//! 复用检测 + 恢复计次
//!
//! 复用检测策略（v2 — Phase 3.7）:
//!   - 主身份: device_id（客户端 UUID，存 IndexedDB/文件/EEPROM）
//!   - 副身份: MAC 地址（可选，仅作日志参考）
//!   - 检测: 同一 SHA256 出现 2+ 不同 device_id → 并发嫌疑
//!   - 单设备换 MAC 不会误报（device_id 不变）

use std::collections::{HashMap, HashSet};

const MAX_RESTORE_PER_DAY: u8 = 2;

#[derive(Debug, Default)]
pub struct AntiAbuse {
    /// (sha256) → 见过的 device_id 集合
    known_devices: HashMap<[u8; 32], HashSet<String>>,
    /// (sha256) → 今日已恢复次数
    restore_count: HashMap<[u8; 32], u8>,
    last_reset_date: String,
}

impl AntiAbuse {
    pub fn new() -> Self { Self::default() }

    /// 检查复用：同一证书是否从不同 device_id 出现
    pub fn check_reuse(&mut self, sha256: &[u8; 32], device_id: Option<&str>) -> ReuseResult {
        let did = device_id.unwrap_or("unknown");
        let devices = self.known_devices.entry(*sha256).or_default();
        let is_new = devices.insert(did.to_string());

        if is_new && devices.len() >= 2 {
            // 同一证书被 2+ 设备使用
            ReuseResult::ConcurrentUse
        } else {
            ReuseResult::Ok
        }
    }

    pub fn try_restore(&mut self, sha256: &[u8; 32], is_root: bool) -> RestoreResult {
        self.maybe_reset_date();
        if is_root { return RestoreResult::Ok; }
        let count = self.restore_count.get(sha256).copied().unwrap_or(0);
        if count >= MAX_RESTORE_PER_DAY { return RestoreResult::ExceededLimit; }
        self.restore_count.insert(*sha256, count + 1);
        RestoreResult::Ok
    }

    pub fn restore_count(&self, sha256: &[u8; 32]) -> u8 {
        self.restore_count.get(sha256).copied().unwrap_or(0)
    }

    fn maybe_reset_date(&mut self) {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        if self.last_reset_date != today {
            self.restore_count.clear();
            self.last_reset_date = today;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReuseResult { Ok, ConcurrentUse }
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreResult { Ok, ExceededLimit }

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(n: u8) -> [u8; 32] { let mut h=[0u8;32]; h[0]=n; h }

    #[test]
    fn test_same_device() {
        let mut a = AntiAbuse::new();
        assert_eq!(a.check_reuse(&hash(1), Some("dev-a")), ReuseResult::Ok);
        assert_eq!(a.check_reuse(&hash(1), Some("dev-a")), ReuseResult::Ok);
    }

    #[test]
    fn test_concurrent_detected() {
        let mut a = AntiAbuse::new();
        assert_eq!(a.check_reuse(&hash(2), Some("dev-a")), ReuseResult::Ok);
        assert_eq!(a.check_reuse(&hash(2), Some("dev-b")), ReuseResult::ConcurrentUse);
    }

    #[test]
    fn test_mac_change_no_false() {
        let mut a = AntiAbuse::new();
        // device_id same, MAC changed — tolerated
        assert_eq!(a.check_reuse(&hash(3), Some("dev-a")), ReuseResult::Ok);
        assert_eq!(a.check_reuse(&hash(3), Some("dev-a")), ReuseResult::Ok);
        // different device_id — flags
        assert_eq!(a.check_reuse(&hash(3), Some("dev-b")), ReuseResult::ConcurrentUse);
    }

    #[test]
    fn test_restore_limit() {
        let mut a = AntiAbuse::new();
        let h = hash(4);
        assert_eq!(a.try_restore(&h, false), RestoreResult::Ok);
        assert_eq!(a.try_restore(&h, false), RestoreResult::Ok);
        assert_eq!(a.try_restore(&h, false), RestoreResult::ExceededLimit);
        assert_eq!(a.try_restore(&h, true), RestoreResult::Ok);
    }
}
