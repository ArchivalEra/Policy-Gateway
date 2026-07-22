# policy-gateway 模块系统（当前实现）

> 核心仅做认证门户 + 上网控制。一切扩展功能都是可选模块。

## 当前模块

| 模块 | 二进制 | 位置 | 说明 |
|------|--------|------|------|
| **portal**（必需） | `policy-gateway` | 闪存 | 认证门户 + CA 引擎 + 权限表 |
| **storage-more**（可选） | `policy-gateway` | 闪存/USB/S3 | 扩展存储后端 (bit 3+): 自定义路径 / S3 / tmpfs |
| **vm**（前置依赖） | `policy-gateway-vm` | 闪存 | 主程序快照/回滚，独立二进制 |

安装顺序: `policy-gateway-vm` → `policy-gateway`。

### portal（`policy-gateway`）

编译: `cargo build --release`

功能:
- CA 引擎 (Ed25519 + AES-256-GCM)
- CSR 接收 → 签名 → pending_confirm → active
- /manager 审批面板
- nftables 双表 (pg_pre + pg_nat, 不碰 fw4, 保留 HW NAT)
- 权限表 (位图 + GC + 2 种查法)
- 恢复系统 (短码 / 证书加密 / 关闭)
- 设备追踪 (MAC 集合, 容忍换 MAC, 检测并发)
- CLI: `perm gc/list/stats`, `init`, `module`

### vm（`policy-gateway-vm`）

编译: `cargo build -p policy-gateway-vm --release`

独立于主程序之外的小二进制。用于:
- `snapshot <name>`   — 快照主程序
- `rollback <name>`   — 回滚主程序
- `verify`            — 校验完整性
- `list`              — 列举快照
- `status`            — 当前状态

## 未来规划（搁置 — 待 Phase 3）

以下功能已设计但未实现:

- **vm-mod**: 模块版本管理器（与 vm 共用指令集）
- **存储后端**: redb 持久化 → 后续适配 CF D2 / S3 / 本地文件
- **模块热加载**: 检测 `/mnt/modules/` 变化自动挂载
- **权限动态增长**: 管理员提交新权限类型 → root 审批追加 bit
- **跨设备沙盒通信**: MCU 间微模块互访
