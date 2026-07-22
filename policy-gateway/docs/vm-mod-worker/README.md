# vm-mod-worker — Pages 端模块管理器

vm-mod-worker 是运行在 Cloudflare Pages 上的模块版本管理器。
类比: 路由器的 `policy-gateway-vm` 管理 policy-gateway 本体版本，vm-mod-worker 管理 Pages 端模块版本。

## 管理对象

vm-mod-worker 安装/升级/回滚的对象是部署到 Cloudflare Pages 的模块（worker function）。

当前可管理的模块:

| 模块 | 说明 |
|------|------|
| `policy-gateway-mirror` | 在 Pages 上镜像路由器的 /manager + /signup + /permissions |

## API

| 端点 | 方法 | 说明 |
|------|------|------|
| `/api/vm-mod/list` | GET | 列举已安装模块 |
| `/api/vm-mod/install` | POST | 安装/升级模块 |
| `/api/vm-mod/rollback` | POST | 回滚模块到指定版本 |
| `/api/vm-mod/snapshot` | POST | 创建当前版本快照 |

## 与 vm-mod 的关系

```
vm-mod (路由器):        管理本地模块 → 安装到 /mnt/usb/modules/
vm-mod-worker (Pages): 管理 Pages 模块 → 部署到 Cloudflare Pages
                       双向同步权限表
```
