# vm-mod-worker — Cloudflare Pages 模块版本管理器

## 架构

```
vm-mod-worker (Pages)                     policy-gateway (路由器)
  │                                            │
  ├── 管理 Pages 端模块版本                     ├── vm-mod (本地模块管理)
  │   └── policy-gateway-mirror                │   └── 管理本地模块
  │       └── 镜像 /signup /manager            │
  │                                            │
  └── 与路由器双向同步权限表 ←───────────────┘
```

## 模块

| 模块 | 安装方式 | 说明 |
|------|----------|------|
| `policy-gateway-mirror` | `vm-mod-worker install policy-gateway-mirror` | 在 Pages 上镜像路由器 /manager + /signup |

## API

| 端点 | 说明 |
|------|------|
| `/api/vm-mod/list` | 列举已安装模块 |
| `/api/vm-mod/install` | 安装/升级模块 |
| `/api/vm-mod/rollback` | 回滚模块 |
| `/api/vm-mod/snapshot` | 创建模块快照 |
