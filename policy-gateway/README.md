# policy-gateway — 路由器端

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
# 本地
cargo build --release

# 路由器 mipsel
cargo build --target mipsel-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort

# VM 独立二进制
cargo build -p policy-gateway-vm --release

# 英文版 (编译时选择)
cargo build --release --features lang-en
```

## 语言

| 编译方式 | 语言 |
|---------|------|
| `cargo build` (默认) | 简体中文 |
| `cargo build --features lang-en` | English |
| CLI/API 运行时 | `PG_LANGUAGE=zh` / `en` 环境变量 |
