# policy-gateway — CA 证书签发网关

> **路由器 CA**: 签发所有客户端证书，两阶段确认。
> **device_id**: 持久设备 ID（隐私保护），允许相同 hostname。
> **单项目**: `policy-gateway`（路由器 Rust + Worker JS）。
> **没有连接证书 = 不能上网**

---

## 一、架构方案

### 核心冻结线 (Phase 3.4)

```
核心权限表: HashMap<[u8;32], PermissionEntry> — 永不变
持久化后端: redb 单文件                       — 永不变
降级模式:  纯内存（redb 不可用时自动）         — 永不变

所有存储拓展 → storage-more 模块（通过 bit claim 注册）
bit 0-2: 核心保留 · bit 3+: 模块声明
模块不可用时其 bit 自动锁定，已有证书不受影响
```

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

#### 证书列表与权限列表解耦

```
证书列表 (谁注册了)         权限目录 (存在哪些权限)       设备权限 (谁有什么)
┌─────────────────┐        ┌────────────────────┐       ┌────────────────────┐
│ SHA256 → hostname│        │ bit 0: connector   │       │ SHA256 → bitmap    │
│ SHA256 → hostname│        │ bit 1: admin       │       │ SHA256 → bitmap    │
│ SHA256 → hostname│        │ bit 2: device      │       │ SHA256 → bitmap    │
└─────────────────┘        │ bit 3: storage:rw  │       └────────────────────┘
                            │ ...                │               ↓
                            └────────────────────┘    有效权限 = bitmap & valid_mask
                                    ↓                        实时生效
                            删除某权限 → mask 变化
                            无需扫描全表，无需定时任务
```

**解耦意味着**：
- 从权限目录中删除一个权限 → `valid_bits_mask()` 立刻变化
- 所有设备的 `has_bit()` 基于 `bitmap & mask` 判断 → **权限自动失效**
- 不需要定时跑解耦程序，不需要修改已存储的位图
- 恢复权限只需加回目录，设备位图还在那里

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

权限表和事件日志**共用同一套 GC 规则**：

```
GC 规则 (基于 event_log 判断，不额外扫描 permissions 表):
  Compromised 超过 7 天 → 删除
   查询 event_log: 该 sha256 最后的事件是 revoke? 且超过 7 天 → 清理

  Pending 超过 24h 无后续事件 → 删除
   查询 event_log: 只有 signup 事件? 且超过 24h → 清理

  注册后 24h 无任何访问 → 删除
   查询 event_log: 没有 approve/heartbeat 事件 → 清理

  event_log 本身:
   保留最近 90 天 → 删除更早的日志
   通过 seq 范围 DELETE，不破坏 append-only 语义

GC 执行:
  定时: 每小时自动运行
  手动: policy-gateway perm gc
  每次 GC 清理会在 event_log 追加 gc_cleanup 事件
```

复用最大化:

```
permissions 表        event_log 表          GC 规则
┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│ 当前状态      │◄─────│ 变更历史       │◄─────│ 查 event_log  │
│ (读频繁)     │      │ (写频繁)      │      │ 决定清理什么  │
│              │      │              │      │              │
│ 设备连接时查  │      │ 审计追溯     │      │ 不需要 extra  │
│ 权限判断     │      │ 离线同步     │      │ 字段或扫描    │
│              │      │ GC 决策源    │      │              │
└──────────────┘      └──────────────┘      └──────────────┘
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



## 八-乙、模块系统

模块系统已从核心剥离，独立为 [`MODULES.md`](MODULES.md)。

核心只做：认证门户 + 权限表 + 上网控制。
一切业务功能（计算、存储、前端、VM）都是可插拔模块。

详见 [MODULES.md](MODULES.md)。

## 九、极限压缩部署（路由器）

```
newifi3 闪存: 10MB                        USB 32GB~1TB
┌──────────────────────┐                ┌──────────────────────────┐
│ ImmortalWrt 系统 ~5MB │                │ /mnt/usb/                │
│                      │                │ ├── policy-gateway/      │
│ 闪存只放 1 个东西:    │                │ │   ├── policy-gateway   │
│                      │                │ │   │   (modelize 版本)   │
│  policy-gateway      │                │ │   ├── modules/         │
│  (minimize, UPX)     │                │ │   │   ├── compute.so   │
│  ~800kB              │                │ │   │   ├── storage.so   │
│                      │                │ │   │   └── vm.so        │
│ 开机 → 解压到 /tmp   │                │ │   ├── frontend/       │
│       → 启动内核      │                │ │   │   └── (React SPA) │
│                      │                │ │   └── config/         │
│ 内核只做:            │                │ ├── share/              │
│  ├── 认证门户        │                │ ├── compute/            │
│  ├── 权限表          │                │ └── backup/             │
│  └── 上网控制        │                └──────────────────────────┘
│                      │
│ 余量 ~4MB 随便浪     │
│                      │
│ 前端: 无（API only） │
│ USB 模块提供 Vite8   │
│ React SPA 可替换     │
└──────────────────────┘

                  开机流程:
                    ① 闪存 minimize 启动 → 认证门户 + 上网控制
                    ② 挂载 USB → 检测到 modelize 版本
                    ③ 可选的: 用 modelize 替换 minimize（热升级）
                    ④ 模块按需加载，不阻塞核心功能
```


## 十、路线图

### 已交付

```
Phase 0.5    门户核心 + 权限表 + GC             ✅
Phase 1.0    nftables 双表 + 恢复系统            ✅
Phase 1.5    CA 引擎 (Ed25519 + sign_csr)       ✅
Phase 2.0    PendingConfirm + hw_id              ✅
Phase 2.2    device_id + 安全审查                ✅
Phase 2.5-3  仓库翻新 + recovery.rs 恢复         ✅
Phase 2.6    MODULES.md 修正 + vm-mod 搁置       ✅
Phase 2.7🚀  VM install + /help + MCU + 全路由   ✅
Phase 2.8    交叉编译 + 路由器实测               ✅
```

### API 一览

| 端点 | 方法 | 说明 |
|------|------|------|
| `/signup` | GET | HTML 申请表单（含 Web Crypto 密钥生成） |
| `/api/signup` | POST | 提交 CSR/证书申请 |
| `/signup/status` | GET | HTML 状态页 |
| `/api/signup/status` | GET | JSON 状态查询 |
| `/manager` | GET | HTML 审批面板 |
| `/api/manager/approve` | POST | 批准/拒绝申请 |
| `/api/manager/pending` | GET | JSON 待审批列表（MCU 用） |
| `/api/cert-confirm` | POST | 两阶段确认证书 |
| `/permissions` | GET | HTML 权限表 |
| `/api/help` | GET | JSON 教程（MCU/浏览器/CLI/headless） |
| `/healthz` | GET | 健康检查 JSON |

### 进行中

```
Phase 3.3    v0.3.3 路由器部署 + storage 抽象      🎯（当前）
```
Phase 2.9    🏗️ 项目重构: CLI 优先 + API 去前端 + 时间戳驱动   ✅
Phase 3.0    统一配置 + rclone CLI + TLS 配置        ✅
Phase 3.1    中英双语 + QUIC 提示                   ✅
Phase 3.2    TLS 框架 + nftables 自动部署 + procd    ✅
```

### 搁置

```
- redb 持久化 (overlayfs 兼容)
- 权限动态增长
- 位图编辑器
- 心跳超时断网（与 MIPS 理念相悖）
```

### 已取消

```
- vm-mod-worker: Worker 端不需要模块管理器。模块直接部署 TypeScript 到 Pages。
- vm-mod（独立二进制）: 模块管理合并入 VM，VM 只负责主程序，模块由 vm-mod 统一管理。
```

## 十六、Phase 4.0 — Dart 跨平台 App

### 动机

任何有屏幕 + 有 AP 能力的设备（手机/平板/PC/单片机带屏）都可以通过此 App 管理网关。

不需要浏览器，不需要 SSH。给非技术用户用的。

### 技术选型

```
语言:          Dart 3.x
框架:          Flutter (mobile) / Dart Frog (CLI 备选)
平台:          Android / iOS / Linux / Windows / macOS
网络层:        HTTP (直接调用 policy-gateway API)
              可选: mqtt (事件推送)
协议:          REST JSON (现有 API 完全不改动)
```

### 功能

```
首屏:
  选择服务器: 扫描局域网 / 手动输入 URL
  连接 → 自动检测是否根管理员 / 管理员 / 普通用户

首页 (管理员):
  待审批列表 (滑动批准/拒绝)
  已授权设备列表 (查看/吊销)
  在线状态 (绿色/灰色圆点)
  快速操作: 批准 / 驳回 / 吊销证书

首页 (普通用户):
  我的证书 (状态: pending_confirm / active / rejected)
  申请新证书 (从 CSR 文件导入或粘贴)

设置页:
  语言切换 (中文 / English)
  主题 (亮/暗)
  服务器 URL + Token 管理
  通知开关

设备管理:
  主机名 + SHA256 + 权限位图 (可读显示)
  硬件平台 + device_id 显示
  最后在线时间
  吊销证书
```

### 与后端的关系

```
零耦合:
  App ──调用 CLI──> policy-gateway / pg
  App 不需要额外服务端支持
  所有功能通过 CLI 实现

架构:
  App (Dart/Flutter)
    ↓ 调用本机 Shell
  policy-gateway / pg CLI
    ↓ HTTP JSON
  policy-gateway API 服务 (路由器/本地)

不需要新增 API:
  App 直接调用:
    pg status                   健康检查
    pg pending                  待审批列表
    pg approve <id>             批准
    pg reject <id>              驳回
    pg cert sign <csr|pubkey>   申请证书
    pg cert status <sha256>     查询证书

  权限校验由 CLI 的 PG_TOKEN 环境变量完成
```

### 文件结构

```
app/
  lib/
    main.dart
    api/
      client.dart        # HTTP 客户端封装
      models.dart        # 数据模型
    pages/
      home.dart          # 首页
      login.dart         # 服务器选择/登录
      pending.dart       # 待审批列表
      devices.dart       # 设备列表
      settings.dart      # 设置
    widgets/
      cert_badge.dart    # 证书状态徽章
      device_card.dart   # 设备卡片
    l10n/
      app_zh.arb         # 中文字符串
      app_en.arb         # 英文字符串
  pubspec.yaml
  android/
  ios/
  linux/
  windows/
```

---

## 十二、Phase 3.0 规划 — nftables 事件驱动 + QUIC 兼容

### 当前架构问题

```
所有 HTTP/HTTPS 请求 → nftables REDIRECT → policy-gateway :8443
                                        → 检查权限
                                        → 允许: nftables 添加 authorized_ips
                                        → 拒绝: 返回 302 /signup

问题:
  ① 每个新连接都经过用户态 portal（即使已被拒绝）
  ② QUIC (UDP 443) → 无法被 REDIRECT（REDIRECT 仅 TCP）
  ③ 流量的首包必须经过用户态，增加延迟
```

### 目标架构（事件驱动）

```
权限变更事件 → nftables 规则更新 | 流量完全不经过用户态 |
                                   
  connector bit 被授予:
    → nftables add element ip pg_pre authorized_ips { 192.168.1.100 }
    → 后续该设备的所有流量 → 硬件转发 (flow offload)
    → 完全绕过 policy-gateway 用户态

  connector bit 被撤销:
    → nftables delete element ip pg_pre authorized_ips { 192.168.1.100 }
    → nftables add element ip pg_pre unauthorized_ips { 192.168.1.100 }
    → 该设备的下一个 SYN → 触发 ct状态 → redirect 到 portal
```

### 详细设计

```
链结构（双表方案保持不变，内部优化）:

table inet pg_pre {
  set authorized_ips { type ipv4_addr; flags dynamic; }
  set unauthorized_ips { type ipv4_addr; flags dynamic; timeout 5m; }
  
  chain forward {
    type filter hook forward priority -1;
    
    # 已授权 → 直接放行（触发 flow offload）
    ip saddr @authorized_ips accept
    
    # 未授权（且在超时期内）→ 跳转到 portal
    ip saddr @unauthorized_ips jump captive_portal
    
    # 未知设备 → 首次 HTTP 请求跳 portal
    tcp dport { 80, 443 } jump redirect_portal
  }
}

优化点:
  ① authorized_ips 更新 → 只改 set，不重建整条链（无连接中断）
  ② unauthorized_ips 带 timeout → 5 分钟后自动过期，不用手动清理
  ③ flow offload: 首包匹配后，后续包完全走硬件，CPU 0 干预
```

### QUIC (UDP 443) 处理

```
QUIC 问题:
  REDIRECT 只支持 TCP。QUIC 是 UDP，不能 redirect。
  
  方案 A (推荐): 未授权设备直接 DROP UDP 443
    → 客户端自动回退到 TCP 443 → 走正常 portal 流程
    → 优点: 极其简单，一行 nftables 规则
    → 缺点: QUIC 无法使用（授权后放行）
    
  方案 B: TPROXY + socat 转发
    → nftables tproxy to :8443 (需要额外策略路由)
    → socat 处理 UDP 转发
    → 优点: 保留 QUIC
    → 缺点: 复杂，MIPS 性能差
    
  方案 C: 在 authorized_ips 中的设备 → UDP 443 放行
    → 其他设备 → UDP 443 drop
    → 授权后自动获得 QUIC 能力
    → 推荐方案，与 TCP 一致
```

### ImmortalWrt / OpenWrt 社区现有方案调查

```
现有方案:
  - luci-app-parentcontrol: 基于 MAC 的家长控制，用 iptables time 模块
    → 太简单，不支持证书认证
  - adblock / simple-adblock: DNS 拦截
    → 完全不同的领域
  - mbedOS: 已内置，可利用
  - 内核 >= 5.15: nftables + flowtable 已支持 HW offload (MT7621)
    → mtk_flow_offload.ko (MediaTek 专用)
    → 可以卸载 TCP/UDP 流量到硬件引擎

结论: 没有直接可用的开源实现。
      现有功能最接近的是 luci-app-parentcontrol，但基于 MAC 不够安全。
      policy-gateway 的做法在社区中没有替代品。
```

### 事件驱动集成点

```
权限变更事件 (已经实现):

  Rust 代码                        nftables
  ─────────                       ────────
  approve(sha256, bitmap)
    → connector bit 变化?
      → 获取设备 IP → nft add element
      
  reject(sha256)
    → nft delete element
    → nft add element @unauthorized_ips { IP timeout 5m }
    
  gc() 清理条目
    → 批量同步 nftables 集合

  注意: IP 获取需要从 DHCP 租赁文件或 arp 表。
        OpenWrt: /tmp/dhcp.leases → hostname → IP 映射
        arp -n → IP → MAC 映射
```

### CLI 命令

```
policy-gateway nft sync     → 将权限表全量同步到 nftables
policy-gateway nft status   → 显示当前 nftables 集合内容
policy-gateway nft flush    → 清空 nftables 集合（重置）
```

### QUIC 测试

```
# 在 authorized 设备上:
curl --http3 https://example.com  (需要 curl 支持 HTTP/3)
# 或
chrome://flags/#enable-quic

# 预期: 授权后 QUIC 正常工作
#       未授权时 QUIC 请求超时 → fallback to TCP
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
| nftables 策略 | 事件驱动，仅权限变更时更新 set。流量不经过用户态。QUIC 用 DROP fallback 到 TCP。|


---

## 十三、Phase 3.1 探索 — TLS 1.3 复用 + ECH

### 当前 HACK: dangerous_configuration

```
当前 policy-gateway 使用 rustls dangerous_configuration 跳过客户端证书验证。
这是一个已知的妥协 — 我们把 mTLS 验证推迟到应用层（查 SHA256 表）。
理由: 路由器闪存 10MB，无法存储完整 CA 链 + CRL。
```

### TLS 1.3 复用方案 （0-RTT / Session Resumption）

TLS 1.3 原生支持两种复用机制，可以大幅减少授权设备的认证开销：

```
机制                     延迟                   服务端状态
──────────────────────────────────────────────────────────────
完整握手 (TLS 1.3)       1-RTT (~50ms)         无
会话恢复 (PSK)           0-RTT (~0ms)          会话票据 (ticket)
0-RTT (early data)       0-RTT                 会话票据 + 防重放窗口
```

#### 方案 A: Session Ticket 做权限缓存

```
设备首次授权:
  TCP 连接 → TLS 握手 → mTLS 证书 → 查 SHA256 表 → OK
  → rustls 签发 Session Ticket（嵌入权限位图快照）

设备后续连接:
  TCP 连接 → TLS 1.3 PSK (0-RTT) → 服务端解密 ticket
  → 提取缓存的权限位图 → 无需再查表 → 直接放行

优点:
  - 授权后连接零额外延迟
  - 服务端无状态（ticket 自包含，加密签发）
  - rustls 原生支持 (ticketer)

缺点:
  - Ticket 最长 7 天（大多实现限制）
  - 权限变更后 Ticket 过期前仍有效（可设置短 TTL 缓解）
```

#### 方案 B: 0-RTT 携带权限证明

```
类似方案 A，但客户端可以在 0-RTT 数据中直接发送请求。
服务端在收到完整 ClientHello 前就开始处理。

适用场景: MCU 传感器定期上报数据
  → 首次连接: 完整握手 + 证书验证 (200ms)
  → 后续连接: 0-RTT 数据直接到达服务器 (0ms)
  → 省掉每次上报的握手延迟
```

#### rustls 实现方式

```rust
// 当前: 每次都查 SHA256 表
fn check_auth(sha256: &[u8; 32]) -> bool {
    table.get(sha256).is_some_and(|e| e.bitmap & CONNECTOR != 0)
}

// Phase 3.1: Session Ticket 携带权限快照
// 设备首次授权后，rustls ticketer 签发 ticket
// ticket 内嵌: { sha256, bitmap, expiry }
// 后续连接: ticketer 解密 ticket → 直接权限检查

// 在 rustls ServerConfig 中开启 ticketer:
let ticketer = rustls::Ticketer::new().unwrap();
server_config.set_ticketer(ticketer);
// ticketer 自动处理签发/验证，无需手动代码
```

### ECH（Encrypted Client Hello）探索

#### 背景

```
ECH 是 TLS 1.3 的扩展（RFC 8871, 8879）。
目的: 加密 ClientHello 中的 SNI，防止中间人看到你访问的网站。

ECH + policy-gateway:
  我们的场景与标准 ECH 相反 — 我们不想隐藏域名，
  我们想把所有未授权设备的流量集中到 portal。
  所以 ECH 对我们的核心场景帮助不大。
```

#### 可能的 ECH 用途

```
场景: 根证书恢复 Worker 端点
  ca.example.com/recover
  → ECH 可以隐藏用户正在访问恢复端点的事实
  → 但不是核心需求，搁置

场景: 公网 Worker 镜像
  如果 Worker 镜像了 /manager 端点
  → ECH 可以防止别人扫描到管理端点
  → 低优先级
```

#### ECH 在路由器场景的局限性

```
① ECH 需要 DNS HTTPS 记录和外部服务端支持
   → 路由器作为服务端，ECH 需要在 rustls 或 nginx 中实现
   → rustls 目前不支持 ECH（OpenSSL 1.1.1+ 有实验性支持）
   
② MIPS 性能:
   ECH 需要额外的 HPKE 解密操作
   对 MT7621 来说，这不是免费的
   
③ 我们不需要隐藏 portal 的存在:
   实际上我们想让未授权设备看到 portal
   ECH 隐藏了 portal 身份 → 适得其反
```

**结论**: ECH 对本项目无用，不做。

### TLS 1.3 复用实施计划

```
Phase 3.1.1: 开启 rustls ticketer
  → ServerConfig::set_ticketer()
  → Ticket 有效期 10 分钟（平衡性能与权限实时性）
  → 权限变更时主动过期 Ticket（通过 custom session store）

Phase 3.1.2: 权限快照嵌入 Ticket
  → 自定义 SessionStore 实现
  → ticket 负载: { sha256, bitmap, issued_at }
  → mTLS 验证后签发 ticket

Phase 3.1.3: MCU 优化
  → 0-RTT 支持需要客户端也支持
  → MCU 端 (esp32/STM32): 评估 mbedTLS 的 TLS 1.3 支持
  → 如果不能 0-RTT，普通 session resumption 也能省 1-RTT

Phase 3.1.4: 混合策略
  → 新设备: 完整 TLS 握手 → 应用层查表
  → 已授权设备: 0-RTT + ticket 权限检查
  → 权限变更: 删除 ticket（强制下次完整握手）
```

### 与 nftables 事件驱动的关系

```
TLS 1.3 复用 和 nftables 事件驱动 是互补的:

nftables 负责: 网络层 — 只允许 authorized_ips 的设备联网
TLS 1.3  负责: 传输层 — 授权设备用 0-RTT 免认证重连

二者结合:
  ① 设备授权 → nftables 添加 IP → TLS ticket 签发
  ② 设备断线重连 → TLS 0-RTT → nftables 已放行 → 无缝恢复
  ③ 权限撤销 → nftables 删除 IP → 设备下次连接被 nftables 拦截
     → 即使有 TLS ticket 也无法联网（网络层被拦）
```

---

## 十四、network-mode-extra 模块规划 — 协议优先 + TLS 模式偏好

### 设计目标

> 一个极轻量的网络优化模块。不阻断任何协议，只做偏好提示。
> 核心原则："prefer, never force" — 即使最优协议不可用，回退路径永远畅通。

```
功能:
  ├── 协议控制: auto / tcp-only / udp-only / quic-only (force)
  │              prefer-tcp / prefer-udp / prefer-quic (soft)
  ├── TLS 模式: prefer-0rtt / prefer-1rtt / prefer-tls13
  │              强制 tls13 / 强制 0rtt (force)
  ├── 优先级标记: DSCP tagging (对 QoL 敏感流量设高优先级)
  └── 回退保障: force 模式下阻断非偏好协议
                 prefer 模式下仅优化，不阻断
  └── 例外列表: 某些设备/端口不受协议限制

实现位置:
  policy-gateway/src/modules/network_mode.rs
  作为 modelize feature 编译，不增加 minimize 核心体积
```

### 协议强制（nftables 层，零开销）

```
原理: 在 pg_pre forward 链中添加策略规则，匹配后直接 accept/drop。

实现的 nftables 规则集:

  # TCP-only 模式 (默认)
  chain forward {
    # 允许 TCP
    ip protocol tcp accept
    # 允许 ICMP (ping 等)
    ip protocol icmp accept
    # 允许已建立的连接 (DNS 等走 UDP 的响应)
    ct state { established, related } accept
    # 其他所有协议 DROP
    drop
  }

  # QUIC-only 模式
  chain forward {
    udp dport 443 accept
    tcp dport 443 accept  # QUIC 回退
    ct state { established, related } accept
    drop
  }

  # 纯 QUIC 模式 (极大减少攻击面)
  chain forward {
    udp dport 443 accept
    drop
  }

切换延迟: 一次 nftables 命令行调用，< 1ms。
无运行时开销: 规则匹配在硬件中完成 (flow offload)。
```

### TLS 模式锁定（rustls 层）

```
原理: 修改 ServerConfig 的兼容性设置，限制允许的 TLS 版本/模式。

  # 允许的 TLS 版本
  当前: TLS 1.2 + TLS 1.3 (默认)
  锁定 1-RTT: 禁用 TLS 1.2，仅 TLS 1.3 (1-RTT 握手)
  锁定 0-RTT: TLS 1.3 + max_early_data_size > 0

  # rustls 实现
  ```rust
  // 当前配置
  let mut config = rustls::ServerConfig::builder()
      .with_safe_defaults()  // TLS 1.2 + 1.3
  
  // 仅 TLS 1.3 (强制 1-RTT)
  let mut config = rustls::ServerConfig::builder()
      .with_protocol_versions(&[&rustls::version::TLS13])
      .unwrap()
  
  // TLS 1.3 + 0-RTT
  let mut config = rustls::ServerConfig::builder()
      .with_protocol_versions(&[&rustls::version::TLS13])
      .unwrap()
  config.max_early_data_size = 0x10000;  // 64KB 0-RTT
  ```

  模块只在 modelize 编译时启用此配置。
  minimize 编译: 保持默认 TLS 1.2 + 1.3 兼容。
```

### 性能评估 (MT7621 / MIPS)

```
方案                   开销                   适用场景
────────────────────────────────────────────────────────────
nftables 协议过滤      零 (HW offload)        协议强制
TLS 1.3 only           -1 次协商往返         高安全性环境
TLS 1.3 + 0-RTT       零 (首包即数据)        内网低延迟
纯 QUIC                UDP 协议             IoT/实时通信

MIPS 压力:
  nftables 规则 → 硬件转发 (mtk_flow_offload) → CPU 0%
  TLS 1.3 完整握手 → 约 200μs (Ed25519) → 可接受
  0-RTT → 约 50μs → 推荐内网场景
```

### CLI 接口

```
policy-gateway network-mode auto                         # 默认: 不干预
policy-gateway network-mode tcp-only                     # force: 仅 TCP
policy-gateway network-mode udp-only                     # force: 仅 UDP
policy-gateway network-mode quic-only                    # force: 仅 QUIC
policy-gateway network-mode prefer-tcp                   # prefer: TCP 优先
policy-gateway network-mode prefer-quic                  # prefer: QUIC 优先
policy-gateway network-mode tls13                        # force: 仅 TLS 1.3
policy-gateway network-mode tls13-0rtt                   # force: 仅 TLS 1.3 + 0-RTT
policy-gateway network-mode prefer-tls13                 # prefer: TLS 1.3 优先
policy-gateway network-mode status                       # 查看当前模式
policy-gateway network-mode reset                        # 恢复 auto
```

### 例外列表

```
某些设备/服务需要特殊处理:

  # DNS 需要 UDP 53 (即使 TCP-only 模式)
  chain forward {
    udp dport 53 accept                      # 例外: DNS
    ip saddr @dns_servers udp dport 53 accept # 例外: 特定 DNS
    ...
  }

例外通过 nftables 集合管理，支持动态增删:
  nft add element ip pg_pre dns_servers { 192.168.1.1 }
  nft delete element ip pg_pre dns_servers { 192.168.1.1 }
```

### 与 Phase 3.0 / 3.1 的关系

```
Phase 3.0: 事件驱动 nftables 框架 ← network-mode-extra 依赖此框架
Phase 3.1: TLS 1.3 Session Ticket  ← network-mode-extra 锁定的 TLS 模式
Phase 3.2: network-mode-extra 模块  ← 协议 + TLS 策略一体化

三层叠加后:
  ① 设备首次连接 → 触发 nftables → 跳转到 portal
  ② 授权后 → IP 加入 authorized_ips → TLS ticket 签发
  ③ network-mode-extra 确保只有允许的协议能通过
```

### 许可

```
network-mode-extra 作为可选模块 (modelize feature):
  minimize:  +0 bytes (不编译)
  modelize:  +~50kB (nftables 规则管理器 + CLI)
  运行时:    零 (规则在 HW 中匹配)
```


---

## 十五、Phase 2.9 重构 — CLI 优先，API 去前端，时间戳驱动

### 核心转变

```
之前: Rust 二进制 = HTTP 服务 + HTML 硬编码 + CLI 附带
之后: Rust 二进制 = 纯 API 守护进程 (HTTP/3 + QUIC)
      CLI = 独立二进制，唯一官方客户端
      前端 = Vite 8 独立仓库，调用 CLI 或直接调 API
```

### 架构

```
┌───────────────────────────────────────────────────┐
│  policy-gateway (守护进程)                          │
│  无 HTML, 纯 JSON API                              │
│  API: /api/signup /api/manager/* /api/cert-confirm │
│  传输: HTTPS (默认) + QUIC (可选)                   │
│  存储: redb (本地) / KV (Worker)                    │
│  事件: 所有操作带 timestamp + signature              │
├───────────────────────────────────────────────────┤
│  pg (CLI, 平台无关)                                 │
│  单一静态二进制, 静编 musl                           │
│  子命令:                                            │
│    pg auth login/logout                             │
│    pg cert sign/revoke/list                         │
│    pg perm grant/revoke/list                        │
│    pg approve/reject --id <rid>                     │
│    pg status --sha256 <hex>                         │
│    pg sync pull/push                                │
│  连接: 通过 HTTPS 或 QUIC 与守护进程通信             │
│  认证: 客户端证书 mTLS 或短期 token                   │
├───────────────────────────────────────────────────┤
│  前端 (独立仓库)                                     │
│  Vite 8 + React                                     │
│  通过 CLI 或直接 API 通信                            │
│  部署到 Cloudflare Pages / USB / R2                 │
└───────────────────────────────────────────────────┘
```

### 时间戳驱动

```
所有操作不可变, 按时间戳排序:

  { operation: "approve", sha256: "...", by: "...", 
    timestamp: 1712345678, signature: "edsig..." }

冲突解决:
  - 同一个 sha256 的多个操作 → 按 timestamp 排序
  - 最终结果 = 最后一个操作的状态
  - revoke 永远赢（即使时间戳更小）

断电恢复:
  启动时重放 redb 或 KV 中的所有事件到最新状态
  不依赖请求顺序, 只依赖时间戳
```

### 隐私保护

```
  ① 设备 ID 用 salted hash, 不存原始标识
  ② 日志只记录操作类型 + 时间戳, 不记录 IP/设备信息
  ③ CLI 默认不保存 token, 每次交互需认证
  ④ 前端页面无追踪, 无分析, 无第三方资源
```

### 实施步骤

```
Phase 2.9.1: CLI 骨架
  → pg auth login/logout (token 管理)
  → pg cert sign (curl 调用 /api/signup 的封装)
  → pg approve/reject (封装 /api/manager/approve)
  → 静态编译, 单文件

Phase 2.9.2: 守护进程 HTML 剥离
  → 移除所有 HTML 处理函数
  → 只保留 JSON API + 静态文件服务 (选配)
  → 移除 Web Crypto 浏览器依赖

Phase 2.9.3: 时间戳事件系统
  → 所有操作改为事件追加
  → 启动时重放到最新状态
  → 同步协议基于事件流

Phase 2.9.4: 前端拆分
  → 新建独立仓库
  → Vite 8 + 响应式
  → 通过 CLI 通信
```
