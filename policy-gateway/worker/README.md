# Worker — Cloudflare 部署

## 架构

```
worker/
├── vm-worker/                  ← 核心恢复 Worker (写死，不需要升级)
│   ├── index.ts                ← /recover + /api/recover/*
│   └── wrangler.toml           ← Pages 配置 (2 KV: RECOVERY + CONFIG)
│
├── policy-gateway-mirror/      ← 可选 mirror 模块 (TypeScript，由 vm-mod 管理)
│   ├── index.js                ← 完整 /signup /manager /permissions
│   ├── wrangler.toml
│   └── README.md
│
├── index.js                    ← (旧版，保留兼容)
└── wrangler.toml               ← (旧版，保留兼容)
```

**核心原则：**
- 恢复 Worker 写死不升级，三种恢复途径不变
- mirror 模块是独立的 TypeScript 文件，直接扔 Pages
- 不需要模块管理器 — Worker 端没有 vm-mod-worker

## 部署

### 核心恢复 Worker

```bash
cd vm-worker
wrangler kv:namespace create RECOVERY
wrangler kv:namespace create CONFIG
# 更新 wrangler.toml 中的 KV ID

# 注入密钥（wrangler.toml 里留空的 3 项，缺一不可！）
wrangler secret put RECOVERY_SALT      # openssl rand -hex 32
wrangler secret put RECOVERY_TOKEN     # openssl rand -hex 32
wrangler secret put GATEWAY_SYNC_TOKEN # 与路由器 PG_GATEWAY_SYNC_TOKEN 一致

# ⚠️ 自检: wrangler secret list 应显示以上 3 项
wrangler deploy
```

### mirror 模块 (可选)

```bash
cd ../policy-gateway-mirror
wrangler deploy
```
