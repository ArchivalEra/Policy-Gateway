# 🔐 policy-gateway

**CA 证书签发网关。没证书不能上网。**

路由器 CA 签发证书 + 两阶段确认 + 持久设备 ID。

```
┌─────────────────────────────────────────────────────────┐
│  浏览器/单片机 → CSR → 路由器 CA 签名 → 存 pending     │
│  → 客户端确认 → active → 设备可以上网                   │
│                                                         │
│  device_id: 隐私保护持久 ID, 允许相同 hostname          │
│  权限: hex 位图, valid_bits_mask 实时生效               │
└─────────────────────────────────────────────────────────┘
```

---

## 文档导航

| 你想干什么 | 看这里 |
|-----------|--------|
| **理解整体架构** | [`PLAN.md`](PLAN.md) — 13 章核心设计 |
| **快速上手/编译** | [`policy-gateway/README.md`](policy-gateway/README.md) |
| **用户手册**（浏览器/curl/ESP32） | [`docs/USER_GUIDE.md`](policy-gateway/docs/USER_GUIDE.md) |
| **交叉编译**（mipsel/arm/本地） | [`docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md) |
| **模块系统** | [`docs/MODULES.md`](policy-gateway/docs/MODULES.md) |
| **维护 + 实验规章** | [`docs/MAINTENANCE.md`](policy-gateway/docs/MAINTENANCE.md) |
| **恢复系统设计**（短码/证书加密/Worker） | [`docs/PHASE1.md`](policy-gateway/docs/PHASE1.md) |
| **设备 ID 策略** | [`docs/DEVICE_ID.md`](policy-gateway/docs/DEVICE_ID.md) |

---

## 版本状态

```
phase0.5   核心门户 + 权限表 + GC
phase1     恢复系统 + CLI + nftables 集成
phase1.1   nftables 双表方案 (pg_pre + pg_nat)
phase1.5   维护手册 + 实验规章
phase1.5-2 CA 引擎 (generate_ca / sign_csr)
phase2.0   PendingConfirm + hw_id / hw_platform
phase2.2   device_id + 安全审查 + confirm 鉴权 ← 当前
```

---

## 技术栈

| 组件 | 技术 |
|------|------|
| 路由器 | Rust（axum + tokio + ring + rustls） |
| 公网节点 | Cloudflare Workers + KV |
| CA 引擎 | Ed25519 + AES-256-GCM + HKDF-SHA256 |
| 存储 | redb（计划中） / in-memory HashMap（当前）|
| 数据库 | 无，权限表 + event_log 双表设计 |
| 恢复系统 | 短码(8位) / 证书加密 / Worker Token |
| 前端 | 无内置（API only），模块化可替换 |

---

## 仓库大小

```
src/      ~3,500 行 Rust，25 测试
worker/   ~300 行 JavaScript
docs/     8 份文档，累计 ~60kB
deploy/   安装/引导脚本
```

---

## 许可证

AGPL-3.0
