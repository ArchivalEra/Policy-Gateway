# Worker — Cloudflare 部署

## 目录结构

```
worker/
├── index.js                    ← 核心 Worker (仅 /recover) — 最小程序
│   ├── /recover                ← HTML 恢复页面
│   ├── /api/recover/verify     ← Token 验证 + 签发
│   └── /api/recover/setup-pin  ← (可选) PIN 设置
│
├── vm-mod-worker/              ← Pages 模块管理器 (独立部署)
│   ├── index.js                ← 模块安装/回滚/快照 API
│   └── wrangler.toml           ← Pages 配置
│
├── policy-gateway-mirror/      ← 镜像模块 (由 vm-mod-worker 管理)
│   ├── index.js                ← /signup /manager /permissions 镜像
│   ├── wrangler.toml           ← Pages 配置 (也可独立部署)
│   └── README.md
│
├── wrangler.toml               ← 核心 Worker 配置
└── README.md                   ← 本文件
```

## 部署方式

### 方式 1: 独立部署核心 Worker (最小程序)

```bash
cd policy-gateway/worker

# 创建 KV 命名空间
wrangler kv:namespace create AUTH_TABLE

# 更新 wrangler.toml 中的 KV ID

# 设置恢复 Token
wrangler secret put RECOVERY_TOKEN

# 部署
wrangler deploy

# 完成后访问
curl https://your-worker.workers.dev/recover
```

### 方式 2: 部署 vm-mod-worker (模块管理器)

```bash
cd policy-gateway/worker/vm-mod-worker

# 创建 KV 命名空间
wrangler kv:namespace create AUTH_TABLE
wrangler kv:namespace create PENDING_QUEUE
wrangler kv:namespace create MODULE_REGISTRY

# 部署
wrangler deploy

# 安装 mirror 模块
curl -X POST https://your-worker.workers.dev/api/vm-mod/install \
  -d '{"name": "policy-gateway-mirror"}'
```

### 方式 3: 独立部署 mirror 模块

```bash
cd policy-gateway/worker/policy-gateway-mirror

wrangler kv:namespace create AUTH_TABLE
wrangler kv:namespace create PENDING_QUEUE
wrangler secret put MANAGER_TOKEN

wrangler deploy

# 访问
curl https://mirror.workers.dev/signup
```

## 部署到 Pages (推荐)

所有 Worker 也可以部署到 Cloudflare Pages Functions:

```bash
# Pages 使用 Functions 路由，不需要 wrangler deploy
# 在 Pages 控制台设置:
#   构建命令: (无)
#   输出目录: worker/
#   Functions 路径: worker/
```
