# 🔐 policy-gateway

**不需要 CA 的证书认证网关。**  
浏览器自签证书 → SHA256 当身份证 → 管理员批权限位图 → 上网 / 跑计算。

```
┌─────────────────────────────────────────────────────────┐
│  唯一项目，一个二进制搞定一切。手机只用浏览器。           │
│                                                         │
│  路由器：mTLS 门卫 + iptables 上网控制 + 复用检测       │
│  Worker：公网入口 + 权限表同步 + 计算任务调度            │
└─────────────────────────────────────────────────────────┘
```

---

## 快速了解

| 概念 | 一句话 |
|------|--------|
| **证书** | 浏览器自签名，不带任何权限标记，只是一张身份证 |
| **权限** | SHA256(证书) → 位图，存在路由器的表里 |
| **上网** | bit0(connector)=1 → iptables 放行 WAN |
| **算力** | bit2(device)=1 → Worker 可派计算任务 |
| **防滥用** | 同一证书出现在不同 MAC 上 → 自动失效 |
| **恢复** | 管理员可恢复，每天每证 2 次，根管理员无视上限 |

---

## 项目结构

```
├── README.md               ← 就是这里
├── LICENSE                 ← AGPL-3.0
├── PLAN.md                 ← 完整架构规划 (12 章)
│
├── policy-gateway/         ← Rust 项目
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs
│   │   ├── tls.rs          ← mTLS + dangerous_configuration
│   │   ├── auth.rs         ← 权限表 + SHA256 查表
│   │   ├── api/            ← HTTP 端点
│   │   │   ├── signup.rs
│   │   │   ├── status.rs
│   │   │   ├── manager.rs
│   │   │   └── sync.rs
│   │   ├── anti_abuse.rs   ← 复用检测 + 恢复计次
│   │   └── worker.rs       ← Cloudflare Worker 逻辑
│   ├── frontend/           ← HTML + 字符串表
│   └── docs/PLAN.md
│
├── scripts/                ← 实用工具脚本
│   ├── ssh_auto.py
│   ├── scp_auto.py
│   ├── verify-build.sh
│   └── describe_image.py
│
└── deploy/                 ← 部署配置
    └── router-opkg.sh
```

---

## 编译

```bash
# 本地开发
cd policy-gateway && cargo build

# 路由器 (mipsel)
rustup target add mipsel-unknown-linux-musl
cargo build --target mipsel-unknown-linux-musl --release
upx --best target/mipsel/release/policy-gateway
```

---

## 路线图

```
Phase 0: ca-backend 手机 CA（浏览器自签证书 + /signup）
Phase 1: policy-gateway 核心（mTLS + 权限表 + iptables）
Phase 2: Worker 公网节点 + 双向同步
Phase 3: 防滥用（复用检测 + 恢复计次）
Phase 4: 计算任务调度
```

详细规划见 [`PLAN.md`](PLAN.md)。

---

## 许可证

AGPL-3.0 — 确保任何改进必须回馈社区。
