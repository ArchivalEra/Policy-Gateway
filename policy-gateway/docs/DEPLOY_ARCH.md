# 统一部署架构 — 单域名多模块

## 域名路由

```
ca.example.com (单域名)
  │
  ├── /recover            ← core Worker (根证书恢复)
  ├── /api/recover/*      ← core Worker
  │
  ├── /signup             ← mirror 模块 (与路由器页面一致)
  ├── /manager             ← mirror 模块
  ├── /permissions         ← mirror 模块
  ├── /api/signup          ← mirror 模块
  ├── /api/manager/*       ← mirror 模块
  ├── /api/cert-confirm    ← mirror 模块
  │
  ├── /vm-mod/*            ← vm-mod-worker (模块管理器)
  │
  └── /sync                ← 同步端点
```

### 核心原则

> 公网和本地页面路径完全一致。用户感知不到后端是路由器还是 Worker。
>
> 本地: http://192.168.1.1:8443/signup
> 公网: https://ca.example.com/signup
> → 一样的外观, 一样的交互, 一样的 URL 路径

### Cloudflare Pages 路由配置

```toml
# wrangler.toml (核心 Worker — 仅 /recover)
routes = [
  { pattern = "ca.example.com/recover", script = "core-worker" },
  { pattern = "ca.example.com/api/recover/*", script = "core-worker" },
]

# wrangler.toml (mirror 模块 — 主站点)
routes = [
  { pattern = "ca.example.com/signup", script = "mirror-module" },
  { pattern = "ca.example.com/manager", script = "mirror-module" },
  { pattern = "ca.example.com/permissions", script = "mirror-module" },
  { pattern = "ca.example.com/api/*", script = "mirror-module" },
]

# wrangler.toml (vm-mod-worker)
routes = [
  { pattern = "ca.example.com/vm-mod/*", script = "vm-mod-worker" },
]
```

## MCU 交互流程

```
MCU (ESP32/STM32)                    路由器 :8443                    Worker/Pages
      │                                   │                             │
      │  POST /mirror/api/signup          │                             │
      │  {pubkey, hostname}               │                             │
      │ ─────────────────────────────────→│                             │
      │                                   │ 路由器在线?                  │
      │                                   ├── 是 → CA 签名 → pending    │
      │                                   ├── 否 → proxy 到 Worker       │
      │                                   │        → 存 KV pending       │
      │                                   │                             │
      │  ← {request_id, status, cert?}    │                             │
      │                                   │                             │
      │  GET /mirror/signup/status?id=X   │                             │
      │ ─────────────────────────────────→│                             │
      │  ← {status: "active", ...}        │                             │
```

## MCU 友好的端点

| 端点 | 方法 | MCU 负担 | 说明 |
|------|------|---------|------|
| `/api/signup` | POST | 低 (只需发 pubkey hex) | paekey 直发，无需 TLS 库 |
| `/api/signup` | POST | 中 (需生成 CSR) | 适用于有 mbedTLS 的 MCU |
| `/api/signup/status?sha256=X` | GET | 低 (固定 URL 查询) | 任何 MCU 都能做 HTTP GET |
| `/api/help?topic=mcu` | GET | 零 (固件内置) | 返回 JSON 教程，MCU 自己解析 |

## 模块部署路径约定

```
单域名下:
  /mirror/*      → policy-gateway-mirror 模块
  /vm/*          → vm-mod-worker 模块管理器
  /recover       → 核心 Worker (根证书恢复)
  /sync          → 同步端点

Cloudflare Pages 路由配置:
  [functions]
  root = "/"
  routes = [
    { pattern = "/recover", script = "core-worker" },
    { pattern = "/mirror/*", script = "mirror-module" },
    { pattern = "/vm/*", script = "vm-mod-worker" },
    { pattern = "/sync", script = "sync-module" },
  ]
```

## 用户体验目标

```
设备插上网线:
  ① 访问任意网站 → 被重定向到 portal
  ② portal 显示:"没证书不能上网，点击申请或查看MCU教程"
  ③ MCU 用户: 用示例 curl 命令，一行搞定
  ④ 浏览器用户: 点"生成本地密钥对"→ 提交 → 等待审批
  ⑤ 管理员在 /manager 点"批准"
  ⑥ 设备自动获得上网权限 (nftables authorized_ips 更新)
  ⑦ 全程: 一个域名, 一致体验, 一行代码搞定
```
