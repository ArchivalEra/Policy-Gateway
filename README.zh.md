# 🔐 policy-gateway — CA 证书签发网关

**没证书不能上网。** 路由器用 Ed25519 签发证书，权限在位图里，两阶段确认后放行。

[![CI](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml)
[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.96+-orange)

[English](https://github.com/ArchivalEra/Worker-Router-Gateway/blob/main/.github/README.md)

```
设备 → CSR → CA 签名(Ed25519) → 客户端确认 → 上网
```

## 快速开始

```bash
git clone https://github.com/ArchivalEra/Worker-Router-Gateway.git
cd policy-gateway
cargo test    # 41 测试, 0 警告
cargo build --release
MANAGER_TOKEN=test ./target/release/policy-gateway serve
```

浏览器打开 `http://localhost:8443/manager?token=test`

## 架构

```
┌────────────────────────────────────────────┐
│  核心 (Rust, event-driven, 零 unsafe)      │
│  CA 引擎       权限表       设备追踪       │
│  Ed25519       位图(u64)    device_id      │
│  sign_csr      GC 规则      复用检测       │
├────────────────────────────────────────────┤
│  signup        confirm      manager        │
│  CSR/pubkey    两阶段确认    审批面板       │
├────────────────────────────────────────────┤
│  CLI            VM           Worker         │
│  perm gc/list  安装/回滚    纯恢复模块     │
└────────────────────────────────────────────┘
```

## 组件

| 组件 | 说明 |
|------|------|
| `policy-gateway` | 主程序: HTTP 服务 + CA 引擎 + CLI |
| `policy-gateway-vm` | 不死鸟: 独立版本管理 (安装/快照/回滚) |
| `worker/vm-worker` | Cloudflare Pages: 仅根证书恢复 |

## 文档

| | |
|------|------|
| 核心架构 | [`PLAN.md`](PLAN.md) |
| 编译指南 | [`policy-gateway/README.md`](policy-gateway/README.md) |
| 用户手册 | [`policy-gateway/docs/USER_GUIDE.md`](policy-gateway/docs/USER_GUIDE.md) |
| 交叉编译 | [`policy-gateway/docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md) |
| 维护规章 | [`policy-gateway/docs/MAINTENANCE.md`](policy-gateway/docs/MAINTENANCE.md) |
| 模块指南 | [`policy-gateway/docs/MODULE_GUIDE.md`](policy-gateway/docs/MODULE_GUIDE.md) |
| MCU 教程 | 运行后访问 `/api/help?topic=mcu` |

## 快速部署 (OpenWrt/ImmortalWrt)

```bash
# 1. 安装 VM
scp policy-gateway-vm root@<路由器IP>:/usr/sbin/
ssh root@<路由器IP> "policy-gateway-vm init"

# 2. 安装主程序 (交叉编译后)
scp policy-gateway root@<路由器IP>:/tmp/
ssh root@<路由器IP> "policy-gateway-vm install /tmp/policy-gateway"

# 3. 启动
ssh root@<路由器IP> "MANAGER_TOKEN=<你的token> policy-gateway serve &"
```

浏览器打开 `http://<路由器IP>:8443/manager?token=<你的token>`

## CLI

```bash
policy-gateway serve              启动网页服务
policy-gateway init               首次设置 (生成根证书 + token)
policy-gateway config edit        交互式配置
policy-gateway config tls edit    TLS 连接配置
policy-gateway perm gc            GC 过期条目
policy-gateway-vm snapshot test   创建快照
policy-gateway-vm rollback test   回滚
```

## 开发状态

| Phase | 内容 |
|-------|------|
| **3.2** 🔄 | **TLS 框架 + nftables 自动部署 + procd 脚本 + vm-worker 合并** |
| 3.1 | i18n 中英双语 + QUIC 提示 |
| 3.0 | 统一配置 + rclone 风格 CLI + TLS 配置 |
