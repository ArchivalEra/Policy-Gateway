# policy-gateway 用户手册

## 路由器首次安装

```bash
# 1. 安装 VM（不死鸟）
scp policy-gateway-vm root@<router-ip>:/usr/sbin/
ssh root@<router-ip> "policy-gateway-vm init"

# 2. 安装主程序
scp policy-gateway root@<router-ip>:/tmp/
ssh root@<router-ip> "policy-gateway-vm install /tmp/policy-gateway"

# 3. 启动
ssh root@<router-ip> "MANAGER_TOKEN=<your-token> policy-gateway &"
```

浏览器打开 `http://<router-ip>:8443/manager?token=<your-token>`

## 设备接入

### 浏览器

1. 打开 `http://<gateway>:8443/signup`
2. 点击「生成本地密钥对」
3. 填写设备名，选择权限模板，提交
4. 等待管理员审批

### MCU（ESP32 / STM32）

```bash
# 生成 Ed25519 密钥对，导出公钥 hex
# POST 到 /api/signup
curl -X POST http://<gateway>:8443/api/signup \
  -H 'Content-Type: application/json' \
  -d '{"pubkey":"<ed25519_hex>","hostname":"esp-sensor","requested":"01"}'
```

返回 `request_id` + `cert_pem`。保存证书到 MCU 存储。

### 无头设备

```bash
# 自动注册脚本
HOST="gateway:8443"
NAME=$(hostname)
openssl req -new -newkey rsa:2048 -nodes \
  -keyout /etc/ssl/device.key -out /tmp/device.csr \
  -subj "/CN=$NAME"
CSR=$(cat /tmp/device.csr)
curl -X POST http://$HOST/api/signup \
  -H 'Content-Type: application/json' \
  -d "{\"csr\":\"$CSR\",\"hostname\":\"$NAME\"}"
```

## 管理操作

- **审批**: 访问 `/manager?token=<token>` → 待审批列表 → 批准/拒绝
- **JSON 查询**: `/api/manager/pending?token=<token>`
- **权限表**: `/permissions`
- **查看状态**: `/signup/status?id=<request_id>`
- **CLI 命令**: `policy-gateway perm gc/list/stats | vm snapshot/rollback/list/status/verify`

## VM 使用

```bash
policy-gateway-vm init              # 初始化备份目录
policy-gateway-vm install /path/to/policy-gateway  # 安装/升级
policy-gateway-vm snapshot pre-upgrade  # 升级前快照
policy-gateway-vm rollback pre-upgrade  # 回滚
policy-gateway-vm list              # 查看所有快照
policy-gateway-vm verify            # 校验完整性
```

## Worker 同步

如果部署了 Cloudflare Worker，可以配置双向同步：

```bash
policy-gateway config edit
  worker_url: https://your-worker.pages.dev
  worker_token: <与 Worker 端 WORKER_TOKEN 一致>
  worker_sync_interval: 300
```

启动后自动同步，离线缓存、上线追补。

## 恢复

### 恢复途径

| 途径 | 依赖 | 说明 |
|------|------|------|
| 短码 | 本地 | 8 位恢复码，SHA256 验证，不依赖 Worker |
| 证书加密 | 本地 | 根证书文件本身可恢复，默认可用 |
| Worker | Cloudflare | `/recover` 页面，Token 验证 |

### 首次初始化

```bash
policy-gateway init              # 生成根证书 (pending)
policy-gateway init --confirm    # 验证恢复途径后激活
```
