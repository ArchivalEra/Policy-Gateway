# Phase 1.5 — 设备身份绑定策略探讨（浏览器版）

> 核心矛盾: 手机浏览器没有文件系统、没有硬件序列号、没有 TPM。
> iOS Safari 限制比 Android Chrome 严格得多。

---

## 一、浏览器环境的能力边界

```
                        Chrome Android   Safari iOS   Firefox
WebCrypto generateKey     ✅               ✅          ✅
WebCrypto exportKey       ✅               ❌          ❌
IndexedDB                 ✅               ✅          ✅
Service Worker            ✅               ✅          ✅
localStorage              ✅               ✅          ✅
navigator.userAgent       ✅ (可伪造)      ✅ (可伪造)   ✅ (可伪造)
screen.*                  ✅ (变)          ✅ (变)      ✅ (变)
hardwareConcurrency       ✅ (2-8)         ✅ (2-6)     ✅ (2-8)
deviceMemory              ✅ (0.25-8)      ❌           ❌
```

**关键发现:**
- iOS Safari 不允许导出私钥 (`extractable: false` 是强制的)
- 所有浏览器都可以生成密钥对并存在 IndexedDB 中
- IndexedDB 的数据可以被用户清除（设置 → Safari → 清除历史记录）
- 没有硬件级常量可以读取（连 CPU 核心数都不够稳定）

---

## 二、浏览器场景的设备绑定策略

### 实际上不需要额外的 Device ID

```
浏览器生成的证书本身就绑定了设备:
  ┌────────────────────────────────────────────────────┐
  │  浏览器生成密钥对                                    │
  │  → 私钥存在 IndexedDB（iOS）或可导出（Android）     │
  │  → 公钥用于签名证书                                  │
  │  → 证书提交到服务器                                  │
  │                                                      │
  │  除非用户主动导出私钥，否则证书无法被其他设备使用      │
  │  (iOS 私钥根本不可导出，Android 可导出但需要用户操作) │
  └────────────────────────────────────────────────────┘
```

对于浏览器用户:
- **证书本身就是设备身份**
- 私钥存储在浏览器沙箱中，其他应用/浏览器无法读取
- iOS: 私钥根本不可导出，这是最强的硬件绑定
- Android: 可导出但需用户确认，可视为用户主动转移凭证

### IndexedDB 设备 ID（备选用）

```
如果需要额外的 Device ID:
  const DEVICE_ID_KEY = 'pg_device_id';
  
  async function getDeviceId() {
    const db = await openDB('policy-gateway', 1, {
      upgrade(db) { db.createObjectStore('config'); }
    });
    let id = await db.get('config', DEVICE_ID_KEY);
    if (!id) {
      id = crypto.randomUUID();
      await db.put('config', id, DEVICE_ID_KEY);
    }
    return id;
  }

  // 设备 ID 存 IndexedDB，清除浏览器数据会丢失
  // 丢失后视为新设备重新注册（不影响现有权限）
```

---

## 三、各客户端方案汇总

```
场景             身份方案                          设备 ID 存储位置
───────────────────────────────────────────────────────────────
浏览器 (iOS)     证书私钥 (不可导出) + IndexedDB    IndexedDB
浏览器 (Android) 证书私钥 (可导出) + IndexedDB      IndexedDB
Linux 服务器     证书文件 + device_id 文件           /etc/policy-gateway/device-id
ESP32            证书固件烧录 + device_id 文件       nvs 分区
STM32/单片机     证书固件烧录 + device_id 文件       EEPROM / OTP
无存储 MCU       仅证书（无设备 ID）                —（每次重新生成）
```

### 核心结论

```
  对于浏览器用户:
    → 证书私钥本身就绑定了设备
    → iOS 私钥不可导出 → 最强设备绑定（物理级别）
    → Android 私钥可导出但需用户操作 → 用户主动转移是合理的
  
  对于 MCU 管理员:
    → 必须支持 PERSISTENT_DEVICE_ID
    → 没有非易失存储的 MCU 只能退化为纯证书认证
  
  检测逻辑:
    → 浏览器: 依赖私钥不可导出特性，不需要额外 Device ID
    → MCU: Device ID + 证书双重验证
    → 同一证书出现在不同 Device ID 或不同浏览器 → 告警但不是自动封禁
    → 根管理员可以手动标记“这是我信任的新设备”来消除告警
```

---

## 四、iPhone 用户丢了证书怎么办？

```
场景: iPhone Safari 清除了数据 → 证书私钥丢失
      （IndexedDB 和 WebCrypto 密钥对都没了）

恢复流程:
  ① 访问 /recover (短恢复码 / 证书加密恢复 / Worker 恢复)
  ② 验证身份后 → 签发新证书
  ③ 新证书 -> 新私钥 -> 新设备 ID
  ④ 权限表更新: 旧 sha256 替换为新 sha256（保留位图）

恢复后旧证书自动失效:
  一旦新证书激活，旧证书被标记为 replaced
  旧证书不能再用于任何操作
  这防止了"已恢复"后旧证书仍然可用的问题
```

---

## 五、最终建议

```
1. 主身份: 证书私钥
   — 浏览器: WebCrypto 生成，私钥不可导出（iOS）或需用户操作导出
   — MCU: 本地生成/烧录，私钥存 flash
   — 不依赖任何硬件序列号

2. 辅助身份: PERSISTENT_DEVICE_ID
   — 浏览器: IndexedDB（iOS 私钥不可导出的情况下这是备选）
   — MCU: flash/nvs/EEPROM
   — 纯辅助功能，仅用于检测同一证书被新设备使用

3. 检测结果不是判决:
   — 检测到不同 device_id → 记录日志，给管理员一个标记
   — 不是自动 compromised（除非管理员配置了自动封禁）
   — 根管理员可选择:"信任此设备"或"吊销此证书"

4. 对于"换 MAC"的误判:
   — 用 device_id 替代 MAC 后，MAC 变化不触发任何检测
   — device_id 唯一可能变化的原因是用户清除了浏览器数据
   — 清除浏览器数据后的处理见上方案四
```

以前的 MAC 方案已废弃。新的三层身份体系:

```
证书私钥 (主) ← 不可伪造
    ↓
PERSISTENT_DEVICE_ID (辅) ← IndexedDB / 文件系统
    ↓
MAC 地址 (日志) ← 仅用于调试和统计，不做判定
```
