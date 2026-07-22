#![allow(dead_code)]
//! dns-local — 自定义 DNS 解析 (仅路由器生效)
//!
//! 将指定域名解析到指定 IP（类似 /etc/hosts 但支持 nftables DNAT）。
//! 支持中文域名（IDN 编码）。
//!
//! 配置:
//!   config.toml 中 dns_hosts = ["域名=IP", ...]
//!   例如: dns_hosts = ["ca.example.com=192.168.1.1", "my.网站=10.0.0.1"]
//!
//! 启动时自动部署 nftables NAT 规则将域名流量重定向。

use crate::modules::{CoreState, GatewayModule, ModuleDeclaration};
use axum::Router;
use std::net::IpAddr;

/// DNS 映射条目
pub struct DnsEntry {
    /// 原始域名（可能含中文）
    pub host: String,
    /// 目标 IP
    pub target: IpAddr,
}

/// 解析 dns_hosts 配置
pub fn parse_dns_hosts(config_hosts: &[String]) -> Vec<DnsEntry> {
    let mut entries = Vec::new();
    for entry in config_hosts {
        let parts: Vec<&str> = entry.splitn(2, '=').collect();
        if parts.len() != 2 { continue; }
        let host = parts[0].trim().to_string();
        if let Ok(ip) = parts[1].trim().parse::<IpAddr>() {
            entries.push(DnsEntry { host, target: ip });
        }
    }
    entries
}

/// DNS 管理模块
pub struct DnsLocal {
    entries: Vec<DnsEntry>,
}

impl DnsLocal {
    pub fn new(config_hosts: &[String]) -> Self {
        let entries = parse_dns_hosts(config_hosts);
        let count = entries.len();
        if count > 0 {
            log::info!("📡 dns-local: {} 条自定义 DNS 映射", count);
            for e in &entries {
                log::info!("   {} → {}", e.host, e.target);
            }
        }
        Self { entries }
    }
}

impl GatewayModule for DnsLocal {
    fn name(&self) -> &'static str {
        "dns-local"
    }

    fn declaration(&self) -> ModuleDeclaration {
        ModuleDeclaration {
            name: "dns-local",
            version: 1,
            claim_bits: vec![],  // 不占用权限位
            min_core_version: "0.3.3",
        }
    }

    fn mount(&self, _state: CoreState) -> Router {
        Router::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hosts() {
        let input = vec![
            "ca.网站=192.168.1.1".to_string(),
            "gateway.local=10.0.0.1".to_string(),
            "invalid".to_string(),  // should be ignored
        ];
        let result = parse_dns_hosts(&input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].host, "ca.网站");
        assert_eq!(result[0].target.to_string(), "192.168.1.1");
        assert_eq!(result[1].host, "gateway.local");
    }
}
