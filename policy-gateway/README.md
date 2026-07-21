# policy-gateway — 路由器端

CA 证书签发网关。一个二进制搞定：认证门户 + CA 引擎 + 权限表 + 不死鸟 VM。

## 模块

| 模块 | 说明 |
|------|------|
| `main.rs` | 入口 + init + CA 密钥启动时生成 |
| `auth.rs` | 权限表 + PendingConfirm + device_id/hw_id |
| `tls.rs` | CA 引擎 (generate_ca / sign_csr / verify) |
| `recovery.rs` | 短码(8位) / 证书加密(AES-256-GCM) / 关闭 |
| `anti_abuse.rs` | device_id 复用检测 + 恢复计次 |
| `store.rs` | redb 持久化存储模块 |
| `vm.rs` | 不死鸟委派层（调用独立 VM 二进制）|
| `api/signup.rs` | CSR/pubkey → CA 签发 → pending_confirm (两阶段) |
| `api/confirm.rs` | 证书确认 (cert-confirm) |
| `api/status.rs` | 支持 id 和 sha256 双查法（HTML + JSON）|
| `api/manager.rs` | 审批面板 + 恒定时间 token 鉴权 + JSON pending |
| `api/sync.rs` | 事件数据库同步协议 |
| `api/help.rs` | MCU/浏览器/CLI/headless 接入教程 |
| `api/permissions.rs` | 权限表 HTML 页面 |
| `modules/` | 门户模块 (core-portal 必需) |
| `worker/` | Cloudflare Worker（仅根证书恢复）|

## 测试

```bash
cargo test           # 31 测试
cargo test tls       # CA 引擎测试
cargo test recovery  # 恢复系统测试
```

## 编译

```bash
# 本地
./build.sh

# 路由器 mipsel
cargo zigbuild --target mipsel-unknown-linux-musl --release

# VM 独立二进制
cargo build -p policy-gateway-vm --release
upx --best target/release/policy-gateway-vm
```
