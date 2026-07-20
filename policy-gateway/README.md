# policy-gateway

单一二进制，包含 mTLS 网关 + 权限表 + 计算调度。

## 目录结构

```
policy-gateway/
├── Cargo.toml
├── src/
│   ├── main.rs          # 入口
│   ├── tls.rs           # mTLS + dangerous_configuration
│   ├── auth.rs          # 权限表 + SHA256 查表
│   ├── api/             # HTTP 端点
│   │   ├── signup.rs    # POST /api/signup
│   │   ├── status.rs    # GET /api/signup/status
│   │   ├── manager.rs   # /manager 审批面板
│   │   └── sync.rs      # /sync 路由器↔Worker
│   ├── anti_abuse.rs    # 复用检测 + 恢复计次
│   └── worker.rs        # Cloudflare Worker 逻辑
├── frontend/            # HTML + 字符串表
│   ├── index.html
│   ├── signup.html
│   ├── manager.html
│   └── strings.json     # 字符串表
└── docs/
    └── PLAN.md
```

## 编译



