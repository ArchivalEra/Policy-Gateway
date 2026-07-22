//! nftables 规则管理 — 自动部署/清理
//!
//! 在 `serve` 启动时调用 `deploy()` 部署双表。
//! 退出时通过 Drop 自动清理。

use std::process::Command;

/// 检查 nft 是否可用
pub fn check_nft() -> bool {
    Command::new("nft")
        .arg("--version")
        .output()
        .is_ok()
}

/// 部署 nftables 双表 (pg_pre + pg_nat)
pub fn deploy() -> Result<(), String> {
    if !check_nft() {
        return Err("nft 命令不可用".into());
    }

    let nft = |args: &[&str]| -> Result<(), String> {
        let out = Command::new("nft")
            .args(args)
            .output()
            .map_err(|e| format!("nft 执行失败: {}", e))?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            // EEXIST (table/set already exists) is OK
            if !stderr.contains("File exists") && !stderr.contains("chain already exists") {
                return Err(format!("nft 错误: {}", stderr.trim()));
            }
        }
        Ok(())
    };

    // ===== inet pg_pre (filter) =====
    nft(&["add", "table", "inet", "pg_pre"])?;
    nft(&["add", "set", "inet", "pg_pre", "authorized_ips", "{ type ipv4_addr; flags dynamic; }"])?;
    nft(&["add", "set", "inet", "pg_pre", "authorized_ips6", "{ type ipv6_addr; flags dynamic; }"])?;
    nft(&["add", "chain", "inet", "pg_pre", "forward", "{ type filter hook forward priority -2; }"])?;
    nft(&["add", "rule", "inet", "pg_pre", "forward", "ip saddr @authorized_ips accept"])?;
    nft(&["add", "rule", "inet", "pg_pre", "forward", "ip6 saddr @authorized_ips6 accept"])?;
    nft(&["add", "rule", "inet", "pg_pre", "forward", "udp dport 443 drop"])?;
    nft(&["add", "rule", "inet", "pg_pre", "forward", "accept"])?;

    // ===== ip pg_nat (NAT) =====
    nft(&["add", "table", "ip", "pg_nat"])?;
    nft(&["add", "set", "ip", "pg_nat", "authorized_ips", "{ type ipv4_addr; flags dynamic; }"])?;
    nft(&["add", "chain", "ip", "pg_nat", "prerouting", "{ type nat hook prerouting priority -150; }"])?;
    nft(&["add", "rule", "ip", "pg_nat", "prerouting",
        "ip saddr != @authorized_ips tcp dport { 80, 443 } redirect to :8443"])?;

    Ok(())
}

/// 清理 nftables 双表
pub fn cleanup() -> Result<(), String> {
    if !check_nft() {
        return Ok(());
    }

    let _ = Command::new("nft")
        .args(["delete", "table", "inet", "pg_pre"])
        .output();
    let _ = Command::new("nft")
        .args(["delete", "table", "ip", "pg_nat"])
        .output();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_nft() {
        // Just verify the function runs without panic
        let _ = check_nft();
    }
}
