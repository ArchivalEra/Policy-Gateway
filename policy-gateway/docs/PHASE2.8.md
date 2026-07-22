# Phase 2.8 — Worker 镜像（完整一致的 HTML + API）

## 核心原则

> Worker 拥有与路由器完全一致的 HTML 页面和完全一致的工作流。
> 用户在 Worker 上的体验与在路由器上完全相同。
> 唯一差异: Worker 没有 CA 私钥，证书签发需要路由器上线后同步。

```
┌──────────────────────────────────────────────────────────┐
│  相同的 HTML + 相同的 API + 相同的权限表                   │
│                                                           │
│  路由器: HTML (Rust) + API (Rust) + CA 签名 + redb        │
│  Worker: HTML (JS/Static) + API (JS) + KV + 无签名        │
│  前端: Vite 8 构建 → 输出 → 分别嵌入路由器/Worker         │
└──────────────────────────────────────────────────────────┘
```

## 一致的工作流

```
路由器和 Worker 的 /signup 页面完全一致（同款 HTML 表单）:

  浏览器打开 http://host:8443/signup    (路由器)
  浏览器打开 https://worker.dev/signup   (Worker)
  → 一样的外观, 一样的交互, 一样的 API

路由器和 Worker 的 /manager 页面完全一致:

  浏览器打开 http://host:8443/manager?token=xxx    (路由器)
  浏览器打开 https://worker.dev/manager?token=xxx   (Worker)
  → 一样的审批面板, 一样的 pending 列表, 一样的操作

路由器和 Worker 的 /api/help 返回完全一致的 JSON。
路由器和 Worker 的 /api/signup/status 返回完全一致的 JSON。
```

## 前端模块的部署方式

```
Vite 8 源文件:
  policy-gateway/docs/frontend-module/
  ├── src/
  │   ├── api.js          ← API 封装
  │   ├── signup.js       ← 证书申请组件
  │   ├── manager.js      ← 审批面板组件
  │   └── status.js       ← 状态查询组件
  ├── index.html          ← 首页
  ├── signup.html         ← 申请页面
  ├── manager.html        ← 审批页面
  ├── status.html         ← 状态页面
  └── vite.config.js      ← Vite 8 配置

构建输出:
  npm run build → dist/
  ├── index.html
  ├── signup.html
  ├── manager.html
  ├── status.html
  └── assets/*.js

部署目标:
  ① 路由器: 挂载到 USB → 从 /mnt/usb/modules/frontend/ 提供
  ② Worker: Cloudflare Pages → 自动部署
  ③ 开发时: Vite dev server → proxy 到路由器或 Worker
```

## 实现步骤

```
Phase 2.8.1: Worker 添加 HTML 页面
  → 将构建后的前端文件部署到 Cloudflare Pages
  → Worker 检测路径 → 提供 HTML 或 API

Phase 2.8.2: Worker 添加 /api/signup + /api/manager/approve
  → Worker 处理 pending 队列 (存入 KV)
  → 路由器上线后同步 → 签名

Phase 2.8.3: 双向同步
  → 路由器启动时 pull Worker KV
  → 权限变更时 push 到 Worker KV
  → Worker 变更时等路由器 poll

Phase 2.8.4: 端到端验证
  → 路由器在线: 前端 → 路由器 API
  → 路由器离线: 前端 → Worker API
  → 路由器恢复: 同步 → 一致性
```
