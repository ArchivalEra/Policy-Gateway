//! 复用检测 + 恢复计次
//!
//! 核心防滥用机制：
//! - 同一 SHA256 来自不同 MAC → compromised
//! - 每日恢复上限 2 次，根管理员无视上限

use std::collections::HashMap;

/// 每日最大恢复次数（非根管理员）
const MAX_RESTORE_PER_DAY: u8 = 2;

#[derive(Debug, Default)]
pub struct AntiAbuse {
    /// (sha256) → 上次连接时记录的 MAC
    pub last_mac: HashMap<[u8; 32], String>,
    /// (sha256) → 今日已恢复次数
    restore_count: HashMap<[u8; 32], u8>,
    /// 记录最后一次重置的日期
    last_reset_date: String,
}

impl AntiAbuse {
    pub fn new() -> Self {
        Self::default()
    }

    /// 检查复用：同一证书是否从不同 MAC 出现
    pub fn check_reuse(&mut self, sha256: &[u8; 32], mac: &str) -> ReuseResult {
        if let Some(last_mac) = self.last_mac.get(sha256) {
            if last_mac != mac {
                return ReuseResult::Compromised;
            }
        }
        self.last_mac.insert(*sha256, mac.to_string());
        ReuseResult::Ok
    }

    /// 尝试恢复
    pub fn try_restore(&mut self, sha256: &[u8; 32], is_root: bool) -> RestoreResult {
        self.maybe_reset_date();

        if is_root {
            return RestoreResult::Ok;
        }

        let count = self.restore_count.get(sha256).copied().unwrap_or(0);
        if count >= MAX_RESTORE_PER_DAY {
            return RestoreResult::ExceededLimit;
        }
        self.restore_count.insert(*sha256, count + 1);
        RestoreResult::Ok
    }

    /// 获取今日恢复次数
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
pub enum ReuseResult {
    Ok,
    Compromised,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RestoreResult {
    Ok,
    ExceededLimit,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(n: u8) -> [u8; 32] {
        let mut h = [0u8; 32];
        h[0] = n;
        h
    }

    #[test]
    fn test_reuse_detection() {
        let mut aa = AntiAbuse::new();
        let h = test_hash(1);
        assert_eq!(aa.check_reuse(&h, "aa:bb:cc"), ReuseResult::Ok);
        assert_eq!(aa.check_reuse(&h, "aa:bb:cc"), ReuseResult::Ok);
        assert_eq!(aa.check_reuse(&h, "dd:ee:ff"), ReuseResult::Compromised);
    }

    #[test]
    fn test_restore_limit() {
        let mut aa = AntiAbuse::new();
        let h = test_hash(2);
        assert_eq!(aa.try_restore(&h, false), RestoreResult::Ok);
        assert_eq!(aa.try_restore(&h, false), RestoreResult::Ok);
        assert_eq!(aa.try_restore(&h, false), RestoreResult::ExceededLimit);
        assert_eq!(aa.try_restore(&h, true), RestoreResult::Ok); // root bypass
    }
}
