# 🔐 policy-gateway

**CA 证书签发网关。没证书不能上网。**

[![CI](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml)
![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)
![Rust](https://img.shields.io/badge/rust-1.96+-orange)

```
浏览器/单片机 → CSR → 路由器 CA 签名 → 客户端确认 → 上网
```

## 快速开始

```bash
git clone https://github.com/ArchivalEra/Worker-Router-Gateway.git
cd policy-gateway
cargo test          # 25 测试
./build.sh          # 编译
MANAGER_TOKEN=test cargo run   # 启动
```

浏览器打开 `http://localhost:8443/manager?token=test`。

## 架构

```
                    ┌───────────────────────────┐
                    │ 浏览器/单片机/curl         │
                    │ CSR + hostname →           │
                    │ ← 签名证书 + sha256        │
                    │ POST /api/cert-confirm     │
                    └──────────┬────────────────┘
                               │
┌──────────────────────────────┴──────────────────────────────┐
│  policy-gateway (路由器 / Cloudflare Worker)                 │
│                                                              │
│  CA 引擎           权限表            设备追踪                 │
│  ┌────────┐     ┌──────────┐    ┌───────────┐              │
│  │Ed25519 ├────→│  位图     │    │ device_id │              │
│  │sign_csr│     │  GC 规则  │    │ MAC 集合  │              │
│  └────────┘     └──────────┘    └───────────┘              │
│                                                              │
│  认证流            管理            恢复                      │
│  ┌────────┐     ┌──────────┐    ┌───────────┐              │
│  │signup  │     │ manager  │    │ short code│              │
│  │confirm │     │ approve  │    │ AES-256   │              │
│  └────────┘     └──────────┘    └───────────┘              │
└──────────────────────────────────────────────────────────────┘
```

## 文档

| 文档 | 位置 |
|------|------|
| 用户手册 | `policy-gateway/docs/USER_GUIDE.md` |
| 核心架构 | `PLAN.md` |
| 编译指南 | `policy-gateway/README.md` |
| 交叉编译（高手向） | `policy-gateway/docs/CROSS_COMPILE.md` |
| 维护规章 | `policy-gateway/docs/MAINTENANCE.md` |

## 技术栈

| 组件 | 选型 |
|------|------|
| 运行时 | Rust (axum + tokio + ring + rustls) |
| CA | Ed25519 + AES-256-GCM + HKDF |
| 公网 | Cloudflare Workers + KV (可选) |
| 测试 | 25 测试、0 警告、CI on push |
| 许可证 | AGPL-3.0 |
