# 🔐 policy-gateway — CA 证书签发网关

**没证书不能上网。** 设备必须提交 CSR 或公钥，路由器用 Ed25519 签名，两阶段确认后放行。

[![CI](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml)
[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.96+-orange)
![MIPS](https://img.shields.io/badge/target-mipsel--24kc-blueviolet)
![Platform](https://img.shields.io/badge/platform-ImmortalWrt%2FOpenWrt-orange)

[English](README.md)

```
设备              路由器 (policy-gateway)
  │                   │
  ├─ CSR/公钥 ──────▶│  CA 签名 (Ed25519)
  │◀──── pending_confirm ─┤
  ├─ 确认 ──────────▶│  激活
  │◀──── 上网 ────────┤
```

## 特点

- **Ed25519 CA** — 路由器自带私钥签名证书，无需外部 CA

---

*这个项目诞生于一块45块闲鱼收的newifi3 D2（MT7621AT，128MB内存，10MB SPI闪存）。每一个设计决策——单二进制、无动态链接、位图权限代替证书链、nftables 代替 iptables、零拷贝事件日志、sub-1MB UPX目标——都是那块硬件逼出来的。如果它能在一块十年前的 MIPS 路由器上跑，它能在任何地方跑。*

---
- **两阶段确认** — 防止网络断开导致的幽灵证书
- **位图权限** — 64 位位图，精细控制（上网/管理/计算/存储等）
- **零 mTLS 开销** — SHA256 白名单查表 (O(1))，无需证书链验证
- **强制门户** — nftables 将无证设备重定向到 `/signup`
- **CLI 客户端** — 零依赖 (`pg` 二进制，纯 TCP)
- **中英双语** — 界面和文档均支持
- **不死鸟 VM** — 独立快照/回滚二进制
- **恢复机制** — Cloudflare Pages Worker 提供根证书恢复 (/recover)

## 快速开始

```bash
git clone https://github.com/ArchivalEra/policy-gateway.git
cd policy-gateway

# 编译 & 测试
cargo test                          # 41 测试, 0 警告
cargo build --release               # ~1.7MB (strip 后)

# 本地运行
MANAGER_TOKEN=test ./target/release/policy-gateway serve
```

打开 `http://localhost:8443/manager?token=test`

### 路由器部署 (MIPS 交叉编译)

```bash
# 交叉编译
rustup target add mipsel-unknown-linux-musl
cargo build --target mipsel-unknown-linux-musl --release -Z build-std

# SCP 到路由器
scp target/mipsel-unknown-linux-musl/release/policy-gateway root@<路由器IP>:/tmp/
ssh root@<路由器IP> "MANAGER_TOKEN=<token> /tmp/policy-gateway serve &"
```

详细教程见 [`docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md)。

## 架构

```
┌─────────────────────────────────────────────────┐
│  policy-gateway (Rust, 事件驱动)                 │
│                                                   │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ CA 引擎   │  │ 权限表       │  │ 事件日志   │ │
│  │ Ed25519   │  │ 位图(u64)    │  │ 仅追加     │ │
│  │ sign_csr  │  │ GC 规则      │  │ 时间戳     │ │
│  └──────────┘  └──────────────┘  └────────────┘ │
│                                                   │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ API      │  │ nftables     │  │ CLI (pg)   │ │
│  │ signup   │  │ pg_pre/pg_nat│  │ 批准       │ │
│  │ manager  │  │ 重定向 80    │  │ 待审批     │ │
│  │ confirm  │  │ 已授权放行    │  │ 状态       │ │
│  └──────────┘  └──────────────┘  └────────────┘ │
│                                                   │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ VM 不死鸟│  │ Worker       │  │ 存储后端   │ │
│  │ 快照     │  │ 证书恢复     │  │ redb/S3    │ │
│  │ 回滚     │  │ /recover     │  │ mirror     │ │
│  └──────────┘  └──────────────┘  └────────────┘ │
└─────────────────────────────────────────────────┘
```

## 硬件要求

| 项目 | 最低 |
|------|------|
| 路由器 | MT7620/MT7621 (MIPS 24kc), x86_64, ARM |
| 闪存 | 2 MB (程序) + 100 KB (数据) |
| 内存 | 4 MB (运行时) |
| 内核 | Linux 5.15+ 支持 nftables |
| 工具链 | Rust 1.96+, zig (交叉编译) |
| 系统 | ImmortalWrt 23.05+, OpenWrt 22.03+ |

## 文档

| 文档 | 链接 |
|------|------|
| 核心架构 (中英混) | [`PLAN.md`](PLAN.md) |
| 编译指南 | [`policy-gateway/README.md`](policy-gateway/README.md) |
| 用户手册 | [`policy-gateway/docs/USER_GUIDE.md`](policy-gateway/docs/USER_GUIDE.md) |
| 交叉编译 | [`policy-gateway/docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md) |
| 维护规章 | [`policy-gateway/docs/MAINTENANCE.md`](policy-gateway/docs/MAINTENANCE.md) |
| 模块指南 | [`policy-gateway/docs/MODULE_GUIDE.md`](policy-gateway/docs/MODULE_GUIDE.md) |
| 网络诊断 | [`policy-gateway/docs/NETWORK.md`](policy-gateway/docs/NETWORK.md) |
| MCU 教程 | 启动后访问 `/api/help?topic=mcu` |

## CLI

```bash
policy-gateway serve              启动网页服务
policy-gateway init               首次设置 (生成 CA + token + TLS 证书)
policy-gateway config edit        交互配置
policy-gateway config tls edit    TLS 连接配置
policy-gateway perm gc            清理过期条目
policy-gateway-vm snapshot test   创建快照
policy-gateway-vm rollback test   回滚
```

## 项目状态

**活跃开发 — Phase 3.3** (v0.3.3)

- ✅ 核心闭环: signup → approve → active
- ✅ Ed25519 CA + 两阶段确认
- ✅ nftables 自动部署 + 清理
- ✅ procd 服务脚本 (ImmortalWrt)
- ✅ CLI 客户端 (零依赖)
- ✅ 中英双语
- ✅ MIPS 交叉编译 (MT7621 实测通过)
- ✅ 存储后端抽象 (redb / S3 / mirror stub)
- 🔄 TLS 监听器 (Phase 3.4)
- 🔄 插件模块 (计算、QUIC)

## 许可证

[AGPL-3.0](LICENSE)
