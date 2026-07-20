# 🔐 policy-gateway

证书认证网关。没证书不能上网。自签名证书 + SHA256 权限表。

```
main     — Phase 0.5 稳定版
phase1   — Phase 1 开发中 (nftables + 恢复系统 + redb)
```

## 今日进度 (7/20)

```
Phase 0.5 ✅ 核心门户 + 权限表 + GC + 用户手册
Phase 1   🚧 恢复系统 + CLI + nftables 网络集成
Phase 1.2 📋 redb 持久化 + Worker 拆半 + 事件仲裁
```

## 项目结构

```
├── policy-gateway/       ← Rust 路由器端
│   ├── src/              ← 19 模块，核心实现
│   ├── vm/               ← 不死鸟独立二进制
│   ├── worker/           ← Cloudflare Worker
│   └── docs/             ← 架构/用户/交叉编译手册
├── scripts/              ← SSH/SCP 工具
├── deploy/               ← OpenWrt 安装脚本
└── .githooks/pre-push    ← main 分支保护 + 敏感文件检查
```

详见 `PLAN.md`、`MODULES.md`、`policy-gateway/docs/USER_GUIDE.md`。
