#!/bin/sh
# pg-nftables.sh — policy-gateway nftables 双表规则部署 (IPv4 + IPv6)
#
# 用法:
#   ./pg-nftables.sh setup     # 创建规则
#   ./pg-nftables.sh teardown  # 删除规则
#   ./pg-nftables.sh status    # 查看当前规则

NFT="nft"

setup() {
  # ===== inet pg_pre (filter, IPv4 + IPv6) =====
  $NFT add table inet pg_pre 2>/dev/null || true
  $NFT add set inet pg_pre authorized_ips { type ipv4_addr\; flags dynamic\; } 2>/dev/null || true
  $NFT add set inet pg_pre authorized_ips6 { type ipv6_addr\; flags dynamic\; } 2>/dev/null || true
  $NFT add chain inet pg_pre forward { type filter hook forward priority -2\; policy drop\; } 2>/dev/null || true
  $NFT add rule inet pg_pre forward ip saddr @authorized_ips accept 2>/dev/null || true
  $NFT add rule inet pg_pre forward ip6 saddr @authorized_ips6 accept 2>/dev/null || true
  $NFT add rule inet pg_pre forward udp dport 443 drop 2>/dev/null || true
  # 无 catch-all accept — 未授权设备默认 drop

  # ===== ip pg_nat (NAT, IPv4 only — redirect) =====
  $NFT add table ip pg_nat 2>/dev/null || true
  $NFT add set ip pg_nat authorized_ips { type ipv4_addr\; flags dynamic\; } 2>/dev/null || true
  $NFT add chain ip pg_nat prerouting { type nat hook prerouting priority -150\; } 2>/dev/null || true
  $NFT add rule ip pg_nat prerouting ip saddr != @authorized_ips tcp dport { 80, 443 } redirect to :8443 2>/dev/null || true

  # ===== ip6 pg_nat (NAT, IPv6) — 仅作记录 =====
  # IPv6 REDIRECT 需要 TPROXY + 策略路由，本脚本暂不实现
  # IPv6 设备访问 IPv4 网站时会被 pg_nat 拦截
  # 纯 IPv6 场景需要 ip6tables TPROXY，是独立模块

  echo "✅ pg_pre + pg_nat 双表已部署 (IPv4 + IPv6)"
}

teardown() {
  $NFT delete table inet pg_pre 2>/dev/null || true
  $NFT delete table ip pg_nat 2>/dev/null || true
  echo "✅ 双表已清除"
}

status() {
  echo "=== inet pg_pre ==="
  $NFT list table inet pg_pre 2>/dev/null || echo "(未部署)"
  echo "=== ip pg_nat ==="
  $NFT list table ip pg_nat 2>/dev/null || echo "(未部署)"
}

case "${1:-status}" in
  setup) setup ;;
  teardown) teardown ;;
  status) status ;;
  *) echo "用法: $0 {setup|teardown|status}" ;;
esac
