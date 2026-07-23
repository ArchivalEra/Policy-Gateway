//! 统一配置系统 — 类似于 rclone 的 config
//!
//! 加载顺序 (后覆盖前):
//!   1. 编译默认值
//!   2. 配置文件 (~/.policy-gateway/config.toml)
//!   3. 环境变量 (PG_*)
//!   4. CLI 参数
//!
//! 配置路径:
//!   Linux:   ~/.policy-gateway/config.toml
//!   macOS:   ~/.policy-gateway/config.toml
//!   Windows: %USERPROFILE%\.policy-gateway\config.toml

/// 计算 SHA256 哈希
pub fn hash_token(token: &str) -> String {
    hex::encode(ring::digest::digest(&ring::digest::SHA256, token.as_bytes()))
}

/// 验证 Token 是否匹配哈希
pub fn verify_token(token: &str, hash: &str) -> bool {
    hash_token(token) == hash
}

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TLS 连接配置文件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsProfile {
    /// 允许纯 HTTP（不安全，仅用于调试/局域网）
    pub allow_http: bool,
    /// 允许 TLS 1.2
    pub allow_tls12: bool,
    /// 允许 TLS 1.3
    pub allow_tls13: bool,
    /// 允许 QUIC (HTTP/3)
    pub allow_quic: bool,
    /// 是否启用客户端证书验证 (mTLS)
    pub mtls_enabled: bool,
}

impl Default for TlsProfile {
    fn default() -> Self {
        Self {
            allow_http: true,
            allow_tls12: true,
            allow_tls13: true,
            allow_quic: false,
            mtls_enabled: false,
        }
    }
}

/// 完整配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 服务器连接地址 (CLI 用)
    pub server_url: String,
    /// 管理 Token (SHA256 哈希，明文仅设置时存在)
    pub manager_token_hash: String,
    /// 监听地址 (服务端用)
    pub listen_addr: String,
    /// 监听端口 (服务端用)
    pub listen_port: u16,
    /// 是否启用 HTML 前端
    pub enable_html: bool,
    /// 数据库路径
    pub db_path: String,
    /// 语言 (zh/en)
    pub language: String,
    /// 存储后端 ("redb" / "s3" / "mirror")
    pub storage_backend: String,
    /// S3 端点 (storage_backend="s3" 时)
    pub storage_endpoint: Option<String>,
    /// S3 Bucket 名
    pub storage_bucket: Option<String>,
    /// Mirror Worker URL (storage_backend="mirror" 时)
    pub mirror_worker_url: Option<String>,
    /// Worker (Pages) URL, 用于同步和数据恢复
    pub worker_url: Option<String>,
    /// Worker 通信 Token（路由器 → Worker 认证用）
    pub worker_token: Option<String>,
    /// 与 Worker 同步间隔（秒），默认 300
    pub worker_sync_interval: u64,
    /// 受监控的网络接口列表（空 = 所有接口）
    /// 例如: ["br-lan", "eth0.2"]
    pub monitor_interfaces: Vec<String>,
    /// 自定义 DNS 映射 (host → IP, 仅在路由器生效)
    pub dns_hosts: Vec<String>,  // 格式: "域名=IP", 如 "ca.网站=192.168.1.1"
    /// 是否允许直接访问网关 IP (不经域名)
    pub allow_direct_ip: bool,
    /// 启用 TLS 传输加密 (需 tls feature + tls_cert/tls_key 配置)
    pub tls_enabled: bool,
    /// 启用崩溃自愈 (需 auto-heal feature, 退出时保留 nftables 规则)
    pub auto_heal_enabled: bool,
    /// TLS 证书路径 (可选)
    pub tls_cert: Option<String>,
    /// TLS 密钥路径 (可选)
    pub tls_key: Option<String>,
    /// TLS 连接配置
    pub tls_profile: TlsProfile,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:8443".into(),
            manager_token_hash: hash_token(""),
            listen_addr: "0.0.0.0".into(),
            listen_port: 8443,
            enable_html: true,
            db_path: "/etc/config/policy-gateway/auth.redb".into(),
            language: "zh".into(),
            tls_cert: None,
            tls_key: None,
            tls_profile: TlsProfile::default(),
            storage_backend: "redb".into(),
            storage_endpoint: None,
            storage_bucket: None,
            mirror_worker_url: None,
            worker_url: None,
            worker_token: None,
            worker_sync_interval: 300,
            monitor_interfaces: vec![],
            dns_hosts: vec![],
            allow_direct_ip: true,
            tls_enabled: false,
            auto_heal_enabled: false,
        }
    }
}

impl Config {
    /// 加载配置 (默认 → 文件 → 环境变量)
    pub fn load() -> Self {
        let mut cfg = Config::default();
        
        // 尝试从配置文件加载
        if let Some(path) = config_path() {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(file_cfg) = toml::from_str::<Config>(&content) {
                        cfg = file_cfg;
                    }
                }
            }
        }

        // 环境变量覆盖
        cfg = cfg.apply_env();

        cfg
    }

    /// 应用环境变量覆盖
    fn apply_env(mut self) -> Self {
        if let Ok(v) = std::env::var("PG_SERVER_URL") { self.server_url = v; }
        if let Ok(v) = std::env::var("PG_MANAGER_TOKEN") { self.manager_token_hash = hash_token(&v); }
        if let Ok(v) = std::env::var("PG_LISTEN_ADDR") { self.listen_addr = v; }
        if let Ok(v) = std::env::var("PG_LISTEN_PORT") { self.listen_port = v.parse().unwrap_or(8443); }
        if let Ok(v) = std::env::var("PG_ENABLE_HTML") { self.enable_html = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_DB_PATH") { self.db_path = v; }
        if let Ok(v) = std::env::var("PG_LANGUAGE") { self.language = v; }
        if let Ok(v) = std::env::var("PG_TLS_CERT") { self.tls_cert = Some(v); }
        if let Ok(v) = std::env::var("PG_TLS_KEY") { self.tls_key = Some(v); }
        if let Ok(v) = std::env::var("PG_ALLOW_HTTP") { self.tls_profile.allow_http = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_ALLOW_TLS12") { self.tls_profile.allow_tls12 = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_ALLOW_TLS13") { self.tls_profile.allow_tls13 = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_ALLOW_QUIC") { self.tls_profile.allow_quic = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_MTLS_ENABLED") { self.tls_profile.mtls_enabled = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_STORAGE") { self.storage_backend = v; }
        if let Ok(v) = std::env::var("PG_STORAGE_ENDPOINT") { self.storage_endpoint = Some(v); }
        if let Ok(v) = std::env::var("PG_STORAGE_BUCKET") { self.storage_bucket = Some(v); }
        if let Ok(v) = std::env::var("PG_MIRROR_URL") { self.mirror_worker_url = Some(v); }
        if let Ok(v) = std::env::var("PG_WORKER_URL") { self.worker_url = Some(v); }
        if let Ok(v) = std::env::var("PG_WORKER_TOKEN") { self.worker_token = Some(v); }
        if let Ok(v) = std::env::var("PG_WORKER_SYNC_INTERVAL") { self.worker_sync_interval = v.parse().unwrap_or(300); }
        if let Ok(v) = std::env::var("PG_MONITOR_INTERFACES") {
            self.monitor_interfaces = v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
        if let Ok(v) = std::env::var("PG_TLS_ENABLED") { self.tls_enabled = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_AUTO_HEAL") { self.auto_heal_enabled = v == "true" || v == "1"; }
        self
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<(), String> {
        let path = config_path().ok_or("无法确定配置目录")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
        }
        let content = toml::to_string_pretty(self).map_err(|e| format!("序列化失败: {}", e))?;
        std::fs::write(&path, content).map_err(|e| format!("写入失败: {}", e))?;
        Ok(())
    }

    /// 交互式配置 (类似 rclone config)
    pub fn interactive(&mut self) {
        println!("🔧 policy-gateway 配置");
        println!("(直接回车保留当前值)");
        println!();
        
        Config::prompt("服务器地址", &mut self.server_url);
        let mut token_str = String::new();
        Config::prompt("管理 Token (留空=不修改)", &mut token_str);
        if !token_str.is_empty() {
            self.manager_token_hash = hash_token(&token_str);
        }
        let mut port = self.listen_port.to_string();
        Config::prompt("监听端口", &mut port);
        self.listen_port = port.parse().unwrap_or(8443);
        let mut html_s = if self.enable_html { "true" } else { "false" }.to_string();
        Config::prompt("启用 HTML (true/false)", &mut html_s);
        self.enable_html = html_s == "true" || html_s == "1";
        Config::prompt("数据库路径", &mut self.db_path);
        Config::prompt("语言 (zh/en)", &mut self.language);
        println!();
        println!("--- TLS 连接配置 ---");
        let mut http_s = if self.tls_profile.allow_http { "y" } else { "n" }.to_string();
        Config::prompt("允许 HTTP (y/n)", &mut http_s);
        self.tls_profile.allow_http = http_s == "y" || http_s == "yes" || http_s == "true" || http_s == "1";
        let mut t12 = if self.tls_profile.allow_tls12 { "y" } else { "n" }.to_string();
        Config::prompt("允许 TLS 1.2 (y/n)", &mut t12);
        self.tls_profile.allow_tls12 = t12 == "y" || t12 == "yes" || t12 == "true" || t12 == "1";
        let mut t13 = if self.tls_profile.allow_tls13 { "y" } else { "n" }.to_string();
        Config::prompt("允许 TLS 1.3 (y/n)", &mut t13);
        self.tls_profile.allow_tls13 = t13 == "y" || t13 == "yes" || t13 == "true" || t13 == "1";
        let mut quic = if self.tls_profile.allow_quic { "y" } else { "n" }.to_string();
        Config::prompt("允许 QUIC (y/n)", &mut quic);
        self.tls_profile.allow_quic = quic == "y" || quic == "yes" || quic == "true" || quic == "1";
        let mut mtls = if self.tls_profile.mtls_enabled { "y" } else { "n" }.to_string();
        Config::prompt("启用 mTLS 客户端证书验证 (y/n)", &mut mtls);
        self.tls_profile.mtls_enabled = mtls == "y" || mtls == "yes" || mtls == "true" || mtls == "1";
        if self.tls_profile.mtls_enabled {
            Config::prompt("TLS 证书路径", self.tls_cert.get_or_insert(String::new()));
            Config::prompt("TLS 密钥路径", self.tls_key.get_or_insert(String::new()));
        }
    }

    /// 交互式提示（公开，用于 CLI 子命令）
    pub fn prompt(label: &str, value: &mut String) {
        use std::io::{stdin, stdout, Write};
        print!("  {} [{}]: ", label, value);
        stdout().flush().ok();
        let mut input = String::new();
        stdin().read_line(&mut input).ok();
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            *value = trimmed.to_string();
        }
    }
}

/// 配置文件路径 (~/.policy-gateway/config.toml)
fn config_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".policy-gateway").join("config.toml"))
}
