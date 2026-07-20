# policy-gateway 模块系统

> 从核心剥离的扩展模块架构。核心只做认证门户，一切业务功能都是模块。

## 八-乙、模块管理系统（VM — Version & Module Manager）

VM 不再是单纯的版本回滚。它是整个系统的**模块生命周期 + 能力分发系统**。

```
VM 管理一切"可以独立装卸的东西":
  ├── 核心模块 (root 专属)        ← 认证、权限表、portal
  ├── 业务模块 (admin 各自管理)   ← 存储、算力、前端界面
  └── 沙盒应用 (device 之间交互)  ← MCU 上的小程序
```

### 分级权限体系

```
root (你)
  ├── VM 整个系统：安装/卸载/禁用任何模块
  ├── 毙掉任何管理员或设备的模块（一键 kill）
  ├── 批准新的权限类型申请
  └── 不受任何限制

admin (你信得过的人)
  ├── VM 自己的模块命名空间（互不干涉）
  │   └── admin A 的模块 → admin B 看不到、管不着
  ├── 在自己的沙盒内自由组合模块
  └── 可以申请新增权限类型（由 root 审批）

device (单片机/算力节点)
  ├── 持有连接证书 → 有权上网
  ├── 持有设备证书 → 有权被委派计算任务
  ├── 沙盒内可以运行微模块（受限）
  └── 一个设备的微模块可以与另一个设备的微模块通信
      └── 经策略引擎鉴权 → 权限表有 "interconnect" bit
```

### 权限列表动态增长

```
不是静态的 7 个 bit。管理员可以提交新型权限申请:

  admin 提交: {"name": "interconnect:udp:6000-7000", "description": "设备间 UDP 通信"}
        ↓
  root 在 /manager 审批 → 追加到 permission_catalog → 分配新 bit

## 根证书恢复机制

恢复方式: **Cloudflare API Token**（取代 one-time PIN）

```
管理员在正常时:
  生成一个 Cloudflare API Token（或任意长期凭证）
  存储在个人的云盘 / 密码管理器（如 Bitwarden）
  该 Token 不经过路由器或 Worker 传输

丢失根证书后:
  访问 Worker /recover 页面
  输入 Cloudflare API Token（或其他预配置的凭证）
  Worker 验证 Token → 签发新根证书
  新证书同步到路由器权限表
  用户下载 .pem → 导入浏览器 → 恢复完成

安全性:
  Token 仅存在用户的云盘，不经网络传输
  量子计算时代也难破解（高熵 Token）
  可随时在 Cloudflare 面板吊销
```

## 前端方案

```
核心思想: 前端是完全可替换的 USB 模块，不塞闪存

闪存 (minimize): 纯无前端，只提供 REST API
USB (modelize):
  ├── Vite 8 / React SPA    ← 替换整个前端体验
  ├── 纯静态 HTML（后备）     ← 极小，可塞闪存
  └── 模块化: 替换前端不改核心

Vite 8 优势:
  - 开发体验极快（HMR 毫秒级）
  - 构建产物极小（tree-shaking + 压缩）
  - 原生 ESM，比 Webpack 快 10x

  React 生态:
  - 组件复用率高
  - 与权限表 View 层天然匹配
  - 构建产物可部署到 R2 / Oracle 对象存储

实施计划:
  Phase 0: 核心只提供 API，前端可后接
  Phase 1: USB 模块中提供 Vite 构建的 React SPA
  Phase 2: 对象存储分发（R2 / Oracle S3）
  Phase 3: 前端热替换（不重启核心）
```


        ↓
  所有设备立即看到新权限，证书可申请该 bit
```

位图仍然是 Hex 变长，新 bit 追加到末尾，旧证书不受影响。

### 模块隔离模型

```
root 的命名空间:     /modules/system/
  ├── core-auth
  ├── captive-portal
  └── permission-catalog

admin A 的命名空间:  /modules/admins/a/
  ├── storage-service
  └── compute-dispatcher

admin B 的命名空间:  /modules/admins/b/
  └── web-ui

device 沙盒:        /sandbox/<device_serial>/
  ├── sensor-reader
  └── relay-ctl

cross-device 通信:
  device-A 的 sensor-reader → 策略引擎查 "interconnect" bit
    → 通过 → 转发到 device-B 的 relay-ctl
    → 拒绝 → 403
```

### 仍然双轨制

路由器和 Worker 同步权限表 + 模块清单。任一活着，整个系统可用。

```
路由器: 管理本地模块 + captive portal + 策略执行
Worker: 管理公网模块 + 跨设备路由 + 权限审批代理
```

### 与旧 VM 的关系

```
旧 "vm rollback" → 新 "模块版本切换"
  每个模块独立版本:
    core-auth@v1.2 → rollback 到 v1.1
    storage-service@v3.0 → rollback 到 v2.8
    不影响其他模块

旧 "vm snapshot" → 新 "全局快照"
  root 可以打全局快照，包含所有模块版本
  恢复快照 = 一键还原所有模块到指定版本
```



## 模块存储后端

```
模块可以来自:
  本地: /mnt/usb/modules/<name>/module.toml
  Worker KV / R2: cloudflare 对象存储
  Oracle S3 / 阿里OSS: 兼容 S3 的对象存储
  网络共享: NFS / CIFS 挂载
  光盘 / 移动硬盘: 只读介质
```

## 模块热加载

```
core 检测到 /mnt/usb/modules/ 变化:
  新模块出现 → 读取 module.toml → 挂载路由
  模块被删除 → 卸载路由
  模块版本变更 → 热替换
```
