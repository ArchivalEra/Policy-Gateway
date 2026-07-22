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

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 完整配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 服务器连接地址 (CLI 用)
    pub server_url: String,
    /// 管理 Token
    pub manager_token: String,
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
    /// TLS 证书路径 (可选)
    pub tls_cert: Option<String>,
    /// TLS 密钥路径 (可选)
    pub tls_key: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:8443".into(),
            manager_token: String::new(),
            listen_addr: "0.0.0.0".into(),
            listen_port: 8443,
            enable_html: true,
            db_path: "/etc/config/policy-gateway/auth.redb".into(),
            language: "zh".into(),
            tls_cert: None,
            tls_key: None,
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
        if let Ok(v) = std::env::var("PG_MANAGER_TOKEN") { self.manager_token = v; }
        if let Ok(v) = std::env::var("PG_LISTEN_ADDR") { self.listen_addr = v; }
        if let Ok(v) = std::env::var("PG_LISTEN_PORT") { self.listen_port = v.parse().unwrap_or(8443); }
        if let Ok(v) = std::env::var("PG_ENABLE_HTML") { self.enable_html = v == "true" || v == "1"; }
        if let Ok(v) = std::env::var("PG_DB_PATH") { self.db_path = v; }
        if let Ok(v) = std::env::var("PG_LANGUAGE") { self.language = v; }
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
        Config::prompt("管理 Token", &mut self.manager_token);
        let mut port = self.listen_port.to_string();
        Config::prompt("监听端口", &mut port);
        self.listen_port = port.parse().unwrap_or(8443);
        let mut html_s = if self.enable_html { "true" } else { "false" }.to_string();
        Config::prompt("启用 HTML (true/false)", &mut html_s);
        self.enable_html = html_s == "true" || html_s == "1";
        Config::prompt("数据库路径", &mut self.db_path);
        Config::prompt("语言 (zh/en)", &mut self.language);
    }

    fn prompt(label: &str, value: &mut String) {
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

/// 配置文件路径
fn config_path() -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".policy-gateway").join("config.toml"))
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        Some(PathBuf::from(home).join(".policy-gateway").join("config.toml"))
    }
    #[cfg(target_os = "windows")]
    {
        let home = std::env::var("USERPROFILE").ok()?;
        Some(PathBuf::from(home).join(".policy-gateway").join("config.toml"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}
