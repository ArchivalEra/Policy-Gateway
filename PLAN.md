# ca.example-gateway — 私有 CA + 证书控制上网网关

> **域名**: `ca.example.com`
> **两个独立项目**: `ca-backend`（手机 CA） + `policy-gateway`（路由器/Worker）
> **证书 = 纯身份，权限 = 位图存表里**
> **没有连接证书 = 不能上网**

---

## 一、架构方案

### 上网控制：不碰 iptables，用接入门户（Captive Portal）

之前的设计是用 iptables 每个设备一条规则来拦网——这会让 MT7621 的**硬件 NAT 加速失效**，NAT 性能从 ~900Mbps 掉到 ~300Mbps。

```
旧方案: iptables DROP per device → HW NAT 失效 ❌
新方案: 接入门户（Captive Portal）→ HW NAT 保留 ✅
```

工作流：

```
设备连 WiFi / 插网线
  ├── DHCP 给 IP（所有设备都给）
  ├── DNS 正常解析（不影响日常使用）
  │
  ├── 设备发起第一个 HTTP 请求
  │   → 路由器 nftables 一条规则 REDIRECT 到 captive portal
  │   → portal 检查: 这个设备有没有 connector 证书?
  │      ├── 有 → 写入 flowtable 白名单 → 后续包走 HW NAT 🚀
  │      └── 无 → 显示 /signup 页面
  │
  ├── HTTPS 请求同理
  │   → REDIRECT 443 → 本地自签证书 → 提示用户接受例外
  │
  └── 已授权的设备: flowtable bypass，完全不经过 CPU
```

只有**一条** nftables 规则，不配硬件 NAT 冲突。已授权的连接直接走 flow offload。

### 模块化架构

```
闪存 (10MB) — 只放核心         USB (32GB~1TB) — 放模块和数据
┌──────────────────────┐      ┌──────────────────────────────┐
│ policy-gateway 核心   │      │ /mnt/usb/modules/             │
│   (Rust 静态二进制)   │      │ ├── web-ui/     → 前端界面   │
│                       │      │ ├── compute/    → 算力调度   │
│  功能:                │      │ ├── storage/    → 文件管理   │
│  ├── mTLS 门卫        │      │ ├── vm/         → 版本快照   │
│  ├── 权限表           │      │ └── ...                      │
│  ├── captive portal   │      │                              │
│  ├── flowtable 管理   │      │ /mnt/usb/data/               │
│  ├── 证书 API         │      │ ├── www/        → 前端源码   │
│  └── 模块加载器       │      │ ├── compute/    → 算力任务   │
└──────────────────────┘      │ └── backup/     → 快照归档   │
                               └──────────────────────────────┘
```


## 二、四种证书

证书只当**身份证**，证书里没有任何权限标记。

```
Certificate:
    Subject: CN = PC-A
    Serial:  01AB...
    Extended Key Usage: clientAuth
    （没有 OU、没有自定义扩展）
```

### 类型

| # | 证书 | 发给谁 | 作用 |
|---|------|--------|------|
| ① | **根证书** | 你（手机） | 证明"我是系统主人" |
| ② | **管理证书** | 你信得过的人 | 证明"我是管理员" |
| ③ | **连接证书** | 普通设备 | 证明"我有权上网" |
| ④ | **设备证书** | 算力节点 | 证明"我是沙盒设备" |

### 你的手机连路由器时

根证书本身**不用于 mTLS 客户端认证**（根私钥只在手机 CA 签名时使用，减少泄露面）。

手机使用自己名下的**连接证书**做 mTLS 客户端认证，路由器查到权限表里该 serial 的位图全 1，赋予所有权限。

```
手机 → mTLS 出示 连接证书 (serial=01)
  → 路由器查表: serial=01 → 位图 FF → 全权限
  → 管理面板 + 上网 全开
```


---

## 三、权限位图表（核心）

### 权限模板（全局固定，可追加）

```
bit 位置:  0           1           2           3              4
权限名:    connector   admin       device      storage:read   storage:write
bit 位置:  5               6             7
权限名:    compute:submit  compute:cancel  (预留)
```

#### 权限目录（统一列表）

所有权限定义在一张表中，各页面复用同一份数据：

| bit | 名称 | 可申请 | 谁可批准 |
|-----|------|--------|---------|
| 0 | connector | ✅ | 管理员 |
| 1 | admin | ❌ | 仅根管理员 |
| 2 | device | ✅ | 管理员 |
| 3 | storage:read | ✅ | 管理员 |
| 4 | storage:write | ✅ | 管理员 |
| 5 | compute:submit | ✅ | 管理员 |
| 6 | compute:cancel | ✅ | 管理员 |

```
MCU 申请示例:
  POST /api/signup
  { "cert": "...", "hostname": "sensor-01", "requested": "05" }
  // 05 = bit0(connector) + bit2(device) → 上网 + 算力节点
```

#### 核心设计：connector ≠ device（上网权与委派权分离）

```
connector (bit 0) ── 谁执行: 路由器 iptables
  ├── 有 connector → WAN 放行，能上网
  └── 无 connector → iptables DROP，只能访问 /signup

device (bit 2) ── 谁执行: Worker 调度器
  ├── 有 device → Worker 可下发计算任务
  └── 无 device → 不是算力节点

两个权限完全独立，路由器离线不影响 task dispatch:

  路由器不在线:
    ├── 新设备不能拿 connector（iptables 在路由器上）
    └── 已有 device 证书的节点 → Worker 直接派任务 🚀

这就是颠覆性设计:
  计算任务不经过路由器，Worker 直接发到设备
  路由器只管上网门卫，Worker 管算力调度
  两者解耦，任何一个挂了不影响另一个
```

#### 新增权限时追加到模板末尾，扩一个 bit 就行。位序约定: LSB0（bit 0 = 最低位，与右移运算 >> 一致）


### 设备权限表

```
┌────────┬──────────┬──────────┬──────────────┐
│ serial │ 主机名    │ 权限位图  │ 状态          │
│        │          │ (Hex)    │              │
├────────┼──────────┼──────────┼──────────────┤
│ 00     │ 手机      │ FF       │ active       │
│        │          │ (全 1)   │              │
│ 01AB   │ PC-A     │ 01       │ active       │
│        │          │ (仅 bit0)│              │
│ 02CD   │ 管理员张  │ 03       │ active       │
│        │          │(bit0+1)  │              │
│ 03EF   │ 算力节点  │ 05       │ active       │
│        │          │(bit0+2)  │              │
│ 04GH   │ PC-B     │ 01       │ compromised  │
└────────┴──────────┴──────────┴──────────────┘

位图值 (LSB0, 单字节):
  0x01 = 0b00000001 -> bit0 (connector)
  0x03 = 0b00000011 -> bit0 + bit1 (connector + admin)
  0x05 = 0b00000101 -> bit0 + bit2 (connector + device)
  0xFF = 0b11111111 -> 全部 8 个权限
```

### 权限表生命周期与垃圾回收

权限表不是无限膨胀的。定期 GC 清理过期条目：

```
GC 规则:
  Compromised/Rejected 条目 → 7 天后自动删除
  Pending 申请              → 24 小时后自动删除
  Active 条目               → 永不自动删除（需手动吊销）

CLI 命令:
  policy-gateway perm gc      # 手动触发 GC
  policy-gateway perm list    # 列出所有条目
  policy-gateway perm stats   # 统计信息

定时 GC:
  后台每 1 小时自动执行一次
  只有清理了条目才打日志，安静运行
```

权限可扩展性:

```
permission_catalog() 是唯一的权限定义源
  新增权限: 追加一行 (bit, 名称, 可申请?, 仅根?)
  旧证书不受影响，新申请自动看到新权限
  删除权限: GC 不会回收 bit 位（bit 位置不变，避免移位混乱）
```

### 查权限

```
fn has_permission(hex: &str, bit_n: usize) -> bool {
    let bytes = hex::decode(hex);
    let byte_idx = bit_n / 8;
    let bit_idx  = bit_n % 8;
    (bytes[byte_idx] >> bit_idx) & 1 == 1
}

-> 三次位运算，纳秒级
```

### 权限表同步

路由器和 Worker 之间双向同步权限表（与 CSR 队列、PRL 一起）：

```
路由器上线时:
  GET /sync/pull  ← 拉 Worker 最新权限表
  比对时间戳合并 → 推回本地新增
  POST /sync/push → 推给 Worker

只要路由器和 Worker 有一个活着，
所有人看到的权限表就是一致的。
```

---

## 四、连接证书防滥用

### 自动失效（证书复用检测）

```
设备用连接证书上网:
  路由器记录 (serial, MAC, IP)

同一证书 serial=01AB 从不同 MAC 出现:
  → 策略引擎判定 "复用"
  → 标记 compromised
  → 该证书的所有连接立即断网
  → 对应设备的 iptables 规则清除
```

### 恢复流程

```
管理员在 /manager 看到:
  ┌────────────────────────────────────────────┐
  │  ⚠ 证书 PC-A (01AB) 已因复用自动失效       │
  │                                            │
  │  今日已恢复: 1/2                            │
  │                                            │
  │  [恢复证书]          [标记为永久吊销]        │
  │                                            │
  │  超过 2 次后:                              │
  │  按钮灰掉 → "请联系根管理员无视上限恢复"      │
  └────────────────────────────────────────────┘

点击"恢复证书":
  ① 校验今日恢复次数 < 2 → 继续
  ② 权限表状态改回 active
  ③ 清零 (serial, MAC) 绑定记录
  ④ 今日恢复次数 +1

每日 24:00: 所有设备的恢复次数归零
```

### 根证书无视上限

```
POST /api/permissions/restore-bypass
  Body: { "serial": "01AB" }
  校验:
    ① 请求者 mTLS 证书 serial == 管理员表里标记为 root 的设备
    ② 请求附带使用根证书私钥对 serial 的签名 (sig)
    ③ 路由器用存根的公钥验签
  三者缺一不可，防止 serial 伪造。
  该端点仅在 ca-backend 本地可用，不暴露到公网
  通过: 无视今日恢复次数，直接恢复
```

---

## 五、设备上网流程

```
设备插网线 / 连 WiFi
  │
  ├── DHCP 获取 IP（所有设备都给，不拦）
  ├── DNS 正常解析（不影响 localsend 之类局域网服务）
  │
  ├── 设备发起第一个 HTTP/HTTPS 请求
  │   → nftables REDIRECT（一条规则）
  │   → captive portal 检查:
  │      ├── 已有 connector 证书? → flowtable 放行 → HW NAT ✅
  │      └── 没有证书? → 显示 /signup 页面
  │
  ├── 有头设备 → 浏览器填主机名 → 提交
  ├── 无头设备 → curl / MCU:
  │     POST /api/signup { "hostname": "sensor-01",
  │       "cert": "-----BEGIN CERTIFICATE-----..." }
  │
  ├── 所有客户端同一入口 → 审批队列 (pending)
  │
  ├── 管理员在 /manager 点同意 → 写入权限表
  │
  ├── 设备轮询 → 返回 approved → 心跳开始
  │
  └── 上网后:
      每 60 秒发心跳 → 维持 flowtable 条目
      心跳超时 180 秒 → 清除 flowtable → captive portal 重新拦截
```

---

## 六、证书申请 + 审批流程

### 通用 API（所有客户端共用）

```
POST /api/signup   ← 提交证书 + 主机名 + 申请的权限
  Body: { "cert": "-----BEGIN CERTIFICATE-----...",
          "hostname": "my-server",
          "requested": "05" }          ← hex 位图，可选，默认 01(connector)
  Response: { "request_id": "uuid", "status": "pending" }
  Errors:
    400 — cert 格式无效
    409 — 该证书 SHA256 已存在（防重入）
    429 — 每来源每小时限 3 次

GET /api/signup/status?id=<request_id>
GET /api/signup/status?sha256=<sha256_of_cert>   ← 两种查法
  Response: { "status": "pending" | "approved" | "rejected",
              "request_id": "uuid",
              "bitmap": "01" | null,
              "reason": "..." | null }

  跨浏览器/跨设备查状态:
    用户在 Chrome 提交后，拿到 request_id
    在 Firefox 打开 /status 粘贴 request_id 或直接提交同一证书
    系统按 SHA256 匹配，状态一致
```

### 字符串表（复用所有 UI 文本）

所有前端文案集中在一张表里，按 key 引用，不硬编码：

```json
{
  "signup.title": "申请上网权限",
  "signup.hostname_label": "主机名",
  "status.pending": "等待管理员审批",
  "status.approved": "已批准，可以上网",
  "status.rejected": "已拒绝: {reason}",
  "status.check_again": "查看状态",
  "error.rate_limit": "提交太频繁，请稍后再试",
  "error.duplicate": "证书已存在，不能重复提交",
  "help.title": "使用教程",
  "help.browser": "浏览器生成证书",
  "help.curl": "curl 命令行",
  "help.mcu": "单片机 / ESP32"
}
```

页面渲染时查表：`{{ "signup.title" }}` → "申请上网权限"。换语言只需换表，不改模板。
极限压缩收益：表可以 gzip 后不足 1kB，模板不含文字只含 key，体积减半。

### 使用教程

`/help` 页面自动根据客户端类型显示对应的教程：

```
浏览器:        教程 ① 打开页面 → ② 填主机名 → ③ 点提交 → ④ 等审批
curl / shell:   教程 curl 命令一行，复制粘贴即可
Python:         教程 5 行脚本，下载即用
ESP32 / MCU:    教程 C 代码片段 + 编译烧录指引
```

每个教程都包含：生成证书 → 提交 → 轮询 → 安装 → 完成。统一模板，只换代码片段。

### 前端的区别

```
浏览器:                    curl / MCU:
  ┌─────────────────┐      ┌──────────────────────┐
  │  /signup (HTML) │      │  POST /api/signup    │
  │  表单填主机名    │      │  curl -X POST ...   │
  │  点击按钮       │      │  轮询 status        │
  │  背后调同一个API │      │                      │
  └─────────────────┘      └──────────────────────┘
           ↑                        ↑
      都调用 POST /api/signup，返回同一种 JSON
```

### 审批

```
管理员在 /manager 看到统一队列:

  ┌────┬──────────┬──────────────────┬──────────────────┐
  │ #  │ 主机名    │ 证书 SHA256       │ 来源             │
  ├────┼──────────┼──────────────────┼──────────────────┤
  │ 1  │ 我的PC   │ abcd1234...      │ Chrome 浏览器    │
  │ 2  │ sensor-01│ ef567890...      │ curl / MCU       │
  │ 3  │ 服务器A  │ 11223344...      │ 浏览器           │
  └────┴──────────┴──────────────────┴──────────────────┘

  admin 看到申请的设备及请求的权限:

  PC-A  请求: connector, storage:rw
        已选: [x] connector  [x] storage:rw  [ ] admin (不可申请)
        [同意已选] [同意全部] [拒绝]

  管理员只能批准自己权限范围内的 bit（admin 权限不可由普通管理员批准）
  根管理员能看到并批准所有 bit
```


## 七、权限表管理（/permissions 页面）

```
所有持证人都能查看。

根证书持有者和管理员能看到"操作"列:

┌──────┬──────────┬──────────┬──────────────────────────────┐
│ host │ 证书类型  │ 权限位图  │ 操作                         │
├──────┼──────────┼──────────┼──────────────────────────────┤
│ 手机  │ 🏠 根   │ FF   │ [吊销] [编辑权限] [无视上限恢复] │
│ 我的PC│ 🔗 连接  │ 01   │ [✕ 吊销] [编辑权限]            │
│ 管理员│ 📋 管理  │ 03    │ [✕ 吊销] [降级为连接]           │
│ NAS   │ 📦 设备  │ 05   │ [✕ 吊销] [✕ 移除 storage:read]│
└──────┴──────────┴──────────┴──────────────────────────────┘

编辑权限:
  弹出位图编辑器 → 勾选/取消权限 bit → 保存
  保存后权限表更新 → 同步到另一个节点
```

---

## 八、同步策略

### 认证与安全

路由器和 Worker 之间使用**彼此的客户端证书做 mTLS 认证**（同步证书使用独立的短期证书链，每 7 天自动轮换），确保同步通道不被中间人攻击。

### 同步数据

```
权限表 (全量 + 增量)
CSR 队列 (pending 申请)
证书缓存 (已签名)
PRL (权限撤销列表)
恢复计次 (serial -> 今日次数)
操作日志
```

### 冲突处理

同一权限在两端同时被修改 -> **last-write-wins**（以较新的时间戳为准，精确到毫秒）。

### 触发

```
① 路由器上线时 -> 立即全量同步
② 权限变更时   -> 增量推，延时 1 秒去重
③ 每 5 分钟     -> 增量保活
```

### 断网后恢复

```
设备因心跳超时被断网:
  ① 设备重新 HTTPS 请求 -> mTLS 握手
  ② 路由器验证证书仍有效且未被 compromised
  ③ 重新添加 iptables 放行规则
  ④ 新的心跳计时开始

  如果证书已被标记 compromised:
    -> 跳转到 /signup?status=compromised
    -> 页面提示"证书已失效，请联系管理员恢复"
```

---


## 八-甲、安全设计

### 私钥保护

```
根私钥 ca.key:
  ┌── Termux 存储: AES-256-CBC 加密文件
  ├── 解密: 启动时由用户输入口令，仅解密到内存
  ├── 备选: Android KeyStore (硬件隔离，指纹/TEE)
  └── 原则: 私钥仅在签名操作时临时解密，用后立即清除内存中的明文
```

### 关键操作认证

| 操作 | 认证方式 | 防伪造措施 |
|------|---------|-----------|
| 吊销/黑名单 | mTLS + 根证书指纹验签 | 双重校验，仅 ca-backend 本地页面 (localhost:8443) 可操作，不开放网络 API |
| 无视上限恢复 | 根证书私钥签名 nonce | 签名绑定 serial，防重放 |
| 管理员审批 | mTLS + 权限表校验 | 审批时额外比对 CSR 公钥指纹 |
| 设备心跳 | 复用 mTLS 会话 | 心跳令牌由会话证书签名 |
| 路由器 ↔ Worker 同步 | mTLS 双向证书 | 短期证书链，7 天自动轮换 |

### 位图冲突安全

Last-write-wins 可能导致已撤销权限被旧数据还原：

```
解决方案: 每次权限变更递增全局版本号
  同步时: 仅接收版本号 > 当前版本的数据
  撤销操作: 版本号 + 1，确保不可被旧数据覆盖
```

### 恢复计数器防篡改

```
依赖系统时钟有风险（路由器无 RTC 电池）:
  ┌── 恢复计数器同时存储时间戳和当日的日期哈希
  ├── 验证: timestamp_hash(日期) == stored_hash
  └── 若不符 → 拒绝恢复，要求手机 ca-backend 重置计数器
```

### 操作日志访问控制

```
日志文件 /var/log/policy-gateway.log:
  权限: 仅 root 可读写
  内容: 记录操作者 serial + 操作类型 + 时间戳
  保留: 自动轮转，保留最近 90 天
```

### HTTPS 劫持兼容

```
DNS 劫持对 HTTPS 请求无效（浏览器会报证书错误）:
  ┌── 方案 A: 路由器自签一张 wildcard 证书用于劫持页面
  ├── 方案 B: 在 DHCP 响应中下发 PAC 代理自动配置
  ├── 方案 C: 用 iptables REDIRECT 将 443 流量转到本地 /signup
  └── 推荐: 方案 C + 自签证书提示用户接受一次例外
```



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


## 九、极限压缩部署（路由器）

```
newifi3 闪存: 10MB
├── ImmortalWrt 系统:   ~5MB
├── 可用空间:           ~5MB
│
├── policy-gateway 占用:
│   ├── policy-gateway (UPX)     800 kB   ← Rust 单二进制
│   ├── 前端页面 (xz)      200 kB   ← 静态 HTML
│   ├── 权限模板 + 配置     5 kB
│   └── 证书 + 密钥        10 kB
│   └── 总计:            ~1 MB
│
├── 余量: ~4 MB

运行模式:
  闪存存压缩包 → 开机解压到 /tmp (tmpfs)
  → policy-gateway 在内存中运行

USB 32GB (独立挂载):
  ├── /mnt/usb/share/      共享目录
  ├── /mnt/usb/compute/    算力沙盒
  └── /mnt/usb/apps/       第三方
```

---

## 十、路线图

```
Phase 0: ca-backend 手机 CA
  [ ] Termux + openssl 生成根证书
  [ ] 实现 /sign /revoke /status /ca.crt API
  [ ] 测试: curl 签发一个证书

Phase 1: policy-gateway 核心（路由器）
  [ ] Rust 单二进制: mTLS 验证 + 权限表
  [ ] 连接证 → iptables 放行 WAN
  [ ] 无连接证 → 全拦截 + 跳转 /signup
  [ ] /signup + /manager 页面
  [ ] 心跳 + 180s 超时断网

Phase 2: 同步 + Worker
  [ ] Cloudflare Worker 部署
  [ ] 路由器 ↔ Worker 双向同步
  [ ] 测试四种网络场景

Phase 3: 防滥用 + 管理
  [ ] 证书复用检测 → auto compromised
  [ ] 恢复计次（每天 2 次上限）
  [ ] 根证书无视上限恢复
  [ ] /permissions 权限表页面
  [ ] 字符串表 + 多客户端教程 (/help)
  [ ] 位图编辑器

Phase 4: 沙盒 + 计算
  [ ] 设备证书 → 沙盒目录分配
  [ ] 计算任务委派
  [ ] 共享库挂载
```

---

## 十一、已定决策

| 决策 | 选择 |
|------|------|
| 证书设计 | 纯身份（CN + serial），权限全在表里 |
| 权限格式 | Hex 变长位图，可无限扩展 bit |
| 项目 | 单项目 `policy-gateway`（路由器/Worker），手机只用浏览器 |
| 联网控制 | connector → iptables 放行 WAN, device → Worker 派任务, 完全解耦 |
| 证书复用 | 检测到同一 serial 不同 MAC → 自动 compromised |
| 恢复限制 | 每日每证 2 次，根证书可无视上限 |
| 路由器运行时 | policy-gateway 单二进制（mipsel musl, UPX ~800kB）|
| 公网节点 | Cloudflare Workers + KV |
| 同步 | 双向，带时间戳合并，任一活着权限表就可见。Worker 离线也可派计算 |
| 沙盒 | 宿主目录隔离，共享库，不建容器 |
| 许可证 | AGPL-3.0 |
