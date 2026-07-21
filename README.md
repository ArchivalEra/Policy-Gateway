# 🔐 policy-gateway

CA 证书签发网关。没证书不能上网。

```
单项目, 单仓库. Rust 路由器 + Cloudflare Worker.
证书 = CA 签发 | 权限 = 位图 | 身份 = device_id
```

## 快速跳转

| 文档 | 位置 |
|------|------|
| 核心架构 | [`PLAN.md`](PLAN.md) |
| 模块系统 | [`MODULES.md`](MODULES.md) |
| Rust 项目 (编译/测试) | [`policy-gateway/README.md`](policy-gateway/README.md) |
| 用户手册 | [`policy-gateway/docs/USER_GUIDE.md`](policy-gateway/docs/USER_GUIDE.md) |
| 交叉编译 | [`policy-gateway/docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md) |
| 维护手册 | [`policy-gateway/docs/MAINTENANCE.md`](policy-gateway/docs/MAINTENANCE.md) |

## 状态

```
phase0.5  核心门户 + 权限表 + GC
phase1    恢复系统 + CLI + nftables 网络集成
phase1.1  nftables 双表方案 (pg_pre + pg_nat)
phase1.5  维护手册 + 实验规章 + Agent 交接
phase1.6  CA 引擎 (generate_ca / sign_csr)
phase2.0  PendingConfirm + hw_id + cert-confirm
phase2.2  device_id + 安全审查 + confirm token 鉴权 (当前)
```

## 分支

```
main        — Phase 0.5 稳定
phase2.2    — Phase 2 开发 (当前)
```

## 编译

```bash
cd policy-gateway && cargo build
```
