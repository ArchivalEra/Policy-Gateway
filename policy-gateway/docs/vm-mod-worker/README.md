# vm-mod-worker — Worker 端模块管理器

## 架构

```
vm-mod-worker 是部署在 Cloudflare Worker/Pages 上的模块管理器。
它负责:
  ① 管理 Worker 端的所有模块（加载/卸载/版本回滚）
  ② 提供 Worker 镜像功能（/manager + /signup，与路由器一致）
  ③ 与路由器的 vm-mod 通信（同步模块列表）

不做什么:
  ❌ 不处理根证书恢复（那是 core Worker 的职责）
  ❌ 不签名证书（没有 CA 私钥）
```

## 目录结构

```
worker/vm-mod-worker/
├── index.js              ← 入口 (module manager + mirror endpoints)
├── modules/
│   └── mirror/
│       ├── signup.js     ← POST /api/signup (存 pending)
│       ├── signup.html   ← GET /signup (同款 HTML)
│       ├── manager.js    ← GET /manager + POST /api/manager/approve
│       ├── status.js     ← GET /api/signup/status
│       └── permissions.js← GET /permissions
├── wrangler.toml         ← Pages 配置
└── README.md             ← 部署文档
```

## 与 core Worker 的关系

```
核心 Worker (index.js):
  /recover             ← 根证书恢复 (唯一内置功能)
  /api/recover/verify  ← Token 验证

vm-mod-worker (独立部署到 Pages):
  /signup              ← 证书申请
  /signup/status       ← 状态查询
  /manager              ← 审批面板
  /api/manager/approve ← 审批操作
  /api/manager/pending ← 待审批列表 (JSON)
  /permissions          ← 权限表
  /api/help             ← 接入教程
```

## 数据流

```
设备 → Worker/Pages → vm-mod-worker → KV (pending)
                                     → KV (权限表)
                   ↕ sync ↕
路由器 → policy-gateway → redb (权限表)
                        → CA 签名 → pending_confirm → KV sync
```

## 与路由器的同步

```
vm-mod-worker 和路由器 vm-mod 共享同一套同步协议:
  /sync/pull  ← 路由器拉取 Worker 变更
  /sync/push  ← Worker 推送到路由器

同步数据:
  - 权限表 (sha256 → PermissionEntry)
  - pending 队列
  - PRL (权限撤销列表)
```
