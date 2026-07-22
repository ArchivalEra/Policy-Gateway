# 模块管理系统 — 完整指南

## 架构

```
                        ┌─────────────────────────┐
                        │  本地 (路由器)            │
                        │                          │
                        │  policy-gateway          │
                        │  ├── vm-mod (模块管理器)  │
                        │  │   ├── install         │
                        │  │   ├── rollback        │
                        │  │   ├── snapshot        │
                        │  │   └── list            │
                        │  │                        │
                        │  └── 模块:                │
                        │      ├── portal (核心)    │
                        │      ├── frontend (UI)    │
                        │      └── network-mode-ext │
                        └─────────────────────────┘
                               ↕ sync
                        ┌─────────────────────────┐
                        │  Cloudflare Pages        │
                        │                          │
                        │  core Worker: /recover   │
                        │                          │
                        │  vm-mod-worker           │
                        │  ├── install             │
                        │  ├── rollback            │
                        │  ├── snapshot            │
                        │  └── list                │
                        │  └── 管理:               │
                        │      └── mirror (模块)   │
                        │          ├── /signup     │
                        │          ├── /manager    │
                        │          └── /permissions│
                        └─────────────────────────┘
```

## 本地模块管理 (vm-mod)

vm-mod 是 policy-gateway 内置的模块版本管理器，通过 CLI 调用。

### 安装模块

```bash
# 从 USB 安装
policy-gateway-vm install /mnt/usb/modules/frontend-v2.bin
# 或
vm install /mnt/usb/modules/network-mode-ext.bin
```

### 快照与回滚

```bash
# 安装前快照
policy-gateway-vm snapshot pre-upgrade

# 安装新版本 (安装后自动创建 fresh-install 快照)
policy-gateway-vm install /tmp/policy-gateway-new

# 出问题了回滚
policy-gateway-vm rollback pre-upgrade

# 查看所有快照
policy-gateway-vm list
```

### 模块存储位置

```
/mnt/usb/modules/
├── frontend/           ← Vite 8 构建的 UI 模块
│   └── index.html
├── network-mode-ext/   ← 网络模式扩展模块
│   └── module.bin
└── gateway/            ← policy-gateway 本体
    └── policy-gateway
```

模块加载路径可通过 `module` CLI 命令查看：

```bash
policy-gateway module
# 加载路径: /mnt/usb/modules/<name>/module.toml
# 对象存储: Worker R2 / Oracle S3 兼容
```

## Pages 模块管理 (vm-mod-worker)

vm-mod-worker 是部署在 Cloudflare Pages 上的模块管理器。
它管理所有部署到 Pages 的 worker function 模块。

### 部署 vm-mod-worker

```bash
# 1. 克隆仓库
git clone https://github.com/ArchivalEra/Worker-Router-Gateway.git
cd policy-gateway/worker/vm-mod-worker

# 2. 创建 KV 命名空间 (首次部署)
wrangler kv:namespace create AUTH_TABLE
wrangler kv:namespace create PENDING_QUEUE
wrangler kv:namespace create MODULE_REGISTRY

# 3. 更新 wrangler.toml 中的 KV ID

# 4. 部署
wrangler deploy
```

### 安装 mirror 模块

vm-mod-worker 部署后，通过其 API 安装 mirror 模块：

```bash
# 通过 vm-mod-worker API 安装 mirror 模块
curl -X POST https://your-worker.workers.dev/api/vm-mod/install \
  -H 'Content-Type: application/json' \
  -d '{"name": "policy-gateway-mirror"}'

# 查看已安装模块
curl https://your-worker.workers.dev/api/vm-mod/list
```

### mirror 模块的回滚

```bash
# 创建当前版本快照
curl -X POST https://your-worker.workers.dev/api/vm-mod/snapshot \
  -H 'Content-Type: application/json' \
  -d '{"name": "policy-gateway-mirror"}'

# 回滚到指定版本
curl -X POST https://your-worker.workers.dev/api/vm-mod/rollback \
  -H 'Content-Type: application/json' \
  -d '{"name": "policy-gateway-mirror", "version": "v0.1.0"}'
```

### mirror 模块独立部署

mirror 模块也可以直接部署到 Pages（不经过 vm-mod-worker 管理）：

```bash
cd policy-gateway/worker/policy-gateway-mirror
wrangler deploy
```

独立部署后，mirror 模块运行在独立的 workers.dev 子域名下。
由 vm-mod-worker 管理时，mirror 模块运行在同一域名下，通过 vm-mod-worker 路由。

## 同步

```
路由器 (redb)  ←→  Worker (KV)

同步内容:
  - 权限表 (sha256 → PermissionEntry)
  - pending 队列
  - PRL (权限撤销列表)

同步触发:
  - 路由器启动时: 全量 pull
  - 权限变更时: 增量 push
  - 每 5 分钟: 定时同步
```

## 模块开发标准

### 本地模块

```bash
# 模块结构
<module-name>/
├── module.toml          ← 模块元数据
│   name = "example"
│   version = "0.1.0"
│   entry = "module.bin"
│   description = "示例模块"
│
├── module.bin           ← 编译产物 (Rust/Go/C)
└── README.md            ← 说明文档

# 安装
cp -r <module-name> /mnt/usb/modules/
vm-mod install /mnt/usb/modules/<module-name>/module.bin
```

### Pages 模块

```bash
# 模块结构
<module-name>/
├── index.js             ← Worker entry (ES module)
├── wrangler.toml        ← Pages 配置
├── package.json         ← 依赖 (可选)
└── README.md            ← 说明文档

# 安装 (通过 vm-mod-worker)
curl -X POST https://vm-mod-worker/api/vm-mod/install \
  -d '{"name": "<module-name>"}'
```
