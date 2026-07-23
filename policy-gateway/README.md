# policy-gateway — 路由器端 (Phase 3.7-5)

**单二进制**: 认证门户 + CA 引擎 + 权限表 + 事件驱动 SSE。

## 模块

| 模块 | 说明 |
|------|------|
| `auth.rs` | 权限表 + PendingConfirm + device_id/hw_id |
| `tls.rs` | CA 引擎 (Ed25519 generate_ca / sign_csr / verify) |
| `config.rs` | rclone 风格配置系统 (toml + env + CLI 三级覆盖) |
| `recovery.rs` | 短码(8位) / 证书加密(AES-256-GCM) / 关闭 |
| `event_log.rs` | 追加式事件日志，时间戳仲裁冲突 |
| `nft.rs` | nftables 自动部署 (pg_pre FORWARD policy-drop) |
| `store.rs` | redb 持久化 / 内存回退 |
| `lang.rs` | 中英双语 + `lang-en` 编译时 feature |
| `api/events.rs` | SSE 实时事件推送（纯事件驱动，零轮询）|
| `api/signup.rs` | CSR/pubkey → CA 签发 → pending_confirm |
| `api/confirm.rs` | 证书两阶段确认 |
| `api/manager.rs` | 审批面板 + Token 鉴权 + JSON API |
| `api/status.rs` | 通过 id/sha256 查询状态 |
| `api/help.rs` | MCU/浏览器/CLI/headless 接入教程 |
| `api/permissions.rs` | 权限表 HTML (需 Token 验证) |
| `modules/portal.rs` | 核心门户路由注册 |
| `modules/storage_more.rs` | 可扩展存储后端 (bit 3+) |
| `modules/dns_local.rs` | 自定义 DNS 映射 |
| `modules/worker_sync.rs` | Worker 双向同步 |

## 测试

```bash
cargo test              # 43 测试, 0 警告
cargo test --features lang-en  # 英文编译测试
```

## 编译

```bash
# 本地 (x86_64)
cargo build --release

# ImmortalWrt / OpenWrt 全架构支持
# mipsel (32-bit MIPS little-endian, 如 MT7620/MT7621)
cargo build --target mipsel-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort

# mips (32-bit MIPS big-endian)
cargo build --target mips-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort

# aarch64 (64-bit ARM, 如 IPQ8074, MT7986)
cargo build --target aarch64-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort

# armv7 (32-bit ARM hard-float)
cargo build --target armv7-unknown-linux-musleabihf --release \
  -Z build-std=core,alloc,std,panic_abort

# x86_64 (64-bit x86)
cargo build --target x86_64-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort

# riscv64 (RISC-V 64-bit)
cargo build --target riscv64gc-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort

# 查看架构列表:
# rustc --print target-list | grep musl

# VM 独立二进制 (native only)
cargo build -p policy-gateway-vm --release

# 英文版 (编译时选择)
cargo build --release --features lang-en
```

## 语言

| 编译方式 | 语言 | 额外模块 |
|---------|------|---------|
| `cargo build` (默认) | 简体中文 | 最小程序 |
| `cargo build --features lang-en` | English | 最小程序 |
| `cargo build --features tls` | 简体中文 | + TLS 传输加密 |
| `cargo build --features auto-heal` | 简体中文 | + 崩溃自愈 |
| `cargo build --features "tls auto-heal lang-en"` | English | + TLS + 自愈 |
