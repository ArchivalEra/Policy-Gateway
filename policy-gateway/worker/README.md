# policy-gateway — Cloudflare Worker 公网节点

## 部署

```bash
# 1. 创建 KV 命名空间
wrangler kv:namespace create AUTH_TABLE
wrangler kv:namespace create PENDING_QUEUE
wrangler kv:namespace create RECOVERY_PINS

# 2. 将输出的 ID 填入 wrangler.toml

# 3. 设置管理令牌
wrangler secret put MANAGER_TOKEN

# 4. 部署
wrangler deploy
```

## Worker 路由

```
/                        状态页
/signup                 证书申请
/manager                审批面板
/recover                根证书恢复
/api/recover/setup-pin  生成恢复 PIN
/api/recover/verify     验证 PIN + 签发新证书
/sync/pull              路由器拉取数据
/sync/push              路由器推送数据
```
