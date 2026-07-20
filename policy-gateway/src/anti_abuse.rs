//! 复用检测 + 恢复计次
pub struct AntiAbuse {
    /// (sha256, mac) → 上次连接的 MAC
    pub last_mac: std::collections::HashMap<[u8; 32], String>,
    /// sha256 → 今日已恢复次数
    pub restore_count: std::collections::HashMap<[u8; 32], u8>,
}

impl AntiAbuse {
    pub fn new() -> Self {
        Self {
            last_mac: std::collections::HashMap::new(),
            restore_count: std::collections::HashMap::new(),
        }
    }
}
