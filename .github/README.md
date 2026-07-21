# 🔐 policy-gateway — CA 证书签发网关

**没证书不能上网。** 路由器用 Ed25519 签发证书，权限在位图里，浏览器确认后放行。

[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](../LICENSE)

```
设备 → CSR → 路由器 CA 签名 → 客户端确认 → 上网
```

## 快速开始

```bash
git clone https://github.com/ArchivalEra/Worker-Router-Gateway.git
cd policy-gateway
cargo test    # 29 测试
./build.sh    # 编译
MANAGER_TOKEN=test cargo run
```

浏览器打开 `http://localhost:8443/manager?token=test`

## 架构

```
┌─────────────────────────────────────────┐
│  CA 引擎       权限表       设备追踪    │
│  Ed25519       位图         device_id   │
│  sign_csr      GC 规则      MAC 集合    │
├─────────────────────────────────────────┤
│  signup        confirm      manager     │
│  CSR 提交      两阶段确认    审批面板    │
└─────────────────────────────────────────┘
```

## 文档

| | |
|------|------|
| 用户手册 | [`policy-gateway/docs/USER_GUIDE.md`](../policy-gateway/docs/USER_GUIDE.md) |
| 核心架构 | [`PLAN.md`](../PLAN.md) |
| 编译指南 | [`policy-gateway/README.md`](../policy-gateway/README.md) |
| 交叉编译 | [`policy-gateway/docs/CROSS_COMPILE.md`](../policy-gateway/docs/CROSS_COMPILE.md) |
| 维护规章 | [`policy-gateway/docs/MAINTENANCE.md`](../policy-gateway/docs/MAINTENANCE.md) |

## CI/CD

推送到任何分支自动运行 `cargo test` + `cargo build --release`。  
Workflow: [`.github/workflows/ci.yml`](./workflows/ci.yml)
