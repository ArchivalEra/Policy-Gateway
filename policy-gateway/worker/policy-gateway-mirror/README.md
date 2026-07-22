# policy-gateway-mirror — Cloudflare Pages 镜像模块

## 说明

由 `vm-mod-worker` 管理的 Pages 模块。提供与路由器一致的 /signup、/manager、/permissions 等页面和 API。

## 安装

```bash
# 通过 vm-mod-worker 安装
curl -X POST https://worker.dev/api/vm-mod/install \
  -H 'Content-Type: application/json' \
  -d '{"name": "policy-gateway-mirror"}'
```

## API

| 端点 | 说明 |
|------|------|
| `GET /signup` | HTML 申请表单（同款） |
| `POST /api/signup` | 接收 CSR/pubkey → 存 pending |
| `GET /signup/status` | 状态查询（HTML + JSON） |
| `GET /manager` | HTML 审批面板（同款） |
| `GET /api/manager/pending` | JSON 待审批列表 |
| `POST /api/manager/approve` | 审批操作 |
| `GET /permissions` | HTML 权限表 |
| `GET /api/help` | JSON 接入教程 |
