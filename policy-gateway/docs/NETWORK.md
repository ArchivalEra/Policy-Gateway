# 网络连接诊断 & 解决方案

## 当前架构

```
Debian Sid (192.168.1.100)
  ├── 默认网关: 192.168.1.1 (newifi3 路由器, ImmortalWrt)
  ├── DNS: 223.5.5.5 (阿里) / 119.29.29.29 (腾讯)
  └── SOCKS5 代理: 127.0.0.1:2080 (用于国外流量)
```

## 诊断结果

| 目标 | 直连 | 通过 SOCKS5 |
|------|------|-------------|
| 国内 HTTP (baidu.com) | ✅ 301 | ✅ 301 |
| 国外 HTTPS (github.com) | ❌ 超时 | ✅ 200 |
| DNS 解析 | ✅ 正常 | — |
| ICMP Ping | ✅ 正常 | — |

## 发现的问题

### 问题 1: nftables REDIRECT 规则 (已修复)

路由器上的 `pg_nat` 表包含规则:
```
ip pg_nat prerouting ip saddr != @authorized_ips tcp dport { 80, 443 } redirect to :8443
```

这导致本机 (192.168.1.100) 的所有 HTTP/HTTPS 流量被重定向到路由器 8443 端口。
由于 `policy-gateway` 未在路由器上运行，8443 端口无响应 → "connection refused"。

**修复:** 从路由器删除 `pg_pre` 和 `pg_nat` 表:
```bash
ssh root@192.168.1.1 -p 22
nft delete table inet pg_pre
nft delete table ip pg_nat
```

### 问题 2: 国外 HTTPS 直连失败

github.com 等国外站点 TCP 连接超时。原因:
- GFW 对境外 HTTPS 流量进行 DPI 干扰
- 需要 SOCKS5 代理隧道化

## 代理使用

```bash
# 开启代理 (SOCKS5)
proxy_on

# 关闭代理
proxy_off

# 单条命令使用代理
curl --proxy socks5h://127.0.0.1:2080 https://github.com

# Git 通过代理推送
git config --global http.proxy socks5h://127.0.0.1:2080
git push
# 用完记得取消
git config --global --unset http.proxy
```

## 注意事项

1. `proxy_on` 设置 `all_proxy=socks5h://127.0.0.1:2080`，国外流量走代理，国内流量直连
2. 代理本身 (127.0.0.1:2080) 是一个 SOCKS5 隧道，运行在本机
3. DNS 通过国内 DNS 直接解析，不经过代理 (socks5h vs socks5)
4. 路由器 nftables 规则部署 `policy-gateway` 后会自动生效
