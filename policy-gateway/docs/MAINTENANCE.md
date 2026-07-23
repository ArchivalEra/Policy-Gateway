# Phase 3.6 — 项目维护手册

> 项目复杂度开始指数上升，没有规矩不成方圆。
> 本文档覆盖: 维护工作流 / Agent 交接 / 实验规章 / 代码规范

---

## 一、仓库纪律

### 分支策略

```
main        ← 最新稳定版，受 pre-push 保护
phase2.x    ← Phase 2 开发分支（当前 phase2.7）
feature/*   ← 功能分支（可选）
```

| 规则 | 说明 |
|------|------|
| **禁止直接推 main** | pre-push hook 会拦截，设 `SKIP_MAIN_CHECK=1` 可绕过 |
| **Phase 1+ 代码在 phase1.x 分支** | 稳定后由 owner 决定是否合并 main |
| **敏感文件绝不允许进仓库** | pre-push 检查: root-key, token, seed.json, .env, SSH 密钥 |

### pre-push 验证清单

推任何分支前，自动检查:

```
1. 当前推送目标不是 main → 是的话拒绝（除非 SKIP_MAIN_CHECK=1）
2. 暂存区无敏感文件匹配
3. cargo test 建议推之前本地运行
```

---

## 二、路由器实验规章

### 安全三原则

```
原则 1: 绝不断网
  实验前确认 SSH 连接稳定
  nftables 规则加错 → 连不上路由器的应急方案:
    fw4 restart (/etc/init.d/fw4 restart)
    物理重启路由器

原则 2: 沙盒优先
  测试程序先跑在 /tmp 内存中，不写入闪存
  实验完毕 rm -rf /tmp/pg-*

原则 3: 清理垃圾
  每次实验后:
    rm -rf /tmp/pg-* /tmp/vm-*
    nft delete table inet pg_pre 2>/dev/null
    nft delete table ip pg_nat 2>/dev/null
```

### 实验流程

```bash
# Step 1: 确保 SSH 稳定
ssh -p 22 root@<router-ip> "echo alive"

# Step 2: 沙盒目录
ssh -p 22 root@<router-ip> "mkdir -p /tmp/pg-test"

# Step 3: 推送二进制
scp -P 22 ./binary root@<router-ip>:/tmp/pg-test/

# Step 4: 测试
ssh -p 22 root@<router-ip> "/tmp/pg-test/binary --help"

# Step 5: 清理
ssh -p 22 root@<router-ip> "rm -rf /tmp/pg-test"

# Step 6: 网络恢复确认
ssh -p 22 root@<router-ip> "ping -c 1 8.8.8.8"
```

### nftables 实验特别警示

```
⚠️  nftables 实验可能会导致网络中断！

安全做法:
  1. 在独立的 SHELL 窗口保持 SSH 登录（不要关闭）
  2. 在新窗口执行 nftables 命令
  3. 如果网络断了 → 在保持的 SSH 窗口执行:
     nft delete table inet pg_pre
     nft delete table ip pg_nat
     /etc/init.d/fw4 restart
  4. 如果 SSH 断了 → 物理重启路由器

绝对不要在生产环境/有用户在用的时候做 nftables 实验。
```

---

## 三、Agent 交接文档

### 项目概览（给新 agent 的 5 分钟速通）

```
policy-gateway 是什么?
  一个 Rust 写的认证网关。没证书不能上网。
  证书 = 浏览器/设备自签名，SHA256 当身份证。
  权限 = 位图，存在红黑树/redb 里。

代码在哪?
  单仓库 ArchivalEra/Worker-Router-Gateway
  main — 最新稳定
  phase2.x — 开发中（当前 phase2.7）

怎么编译?
  cd policy-gateway && cargo build     # 本地开发
  ./build.sh                           # 一键编译全组件 (main + VM)
  ./setup-cross.sh                     # 交叉编译环境 (mipsel)

怎么测试?
  cargo test                           # 29 测试全过, 0 警告

怎么初始化?
  policy-gateway init                  # 首次设置: 生成根证书 + 管理令牌

怎么部署?
  ./deploy/install.sh                  # OpenWrt 安装

路由器在哪?
  root@<router-ip>
  密钥: <your-ssh-key>
  密码: <your-password>

怎么推代码?
  source proxy-method.sh
  git-proxy-push phase1.2
```

### 关键技术债（未完成事项）

| 事项 | 状态 | 说明 |
|------|------|------|
| redb 持久化 | 📋 Phase 2 | 替换 HashMap，事件日志落地 |
| mTLS 集成 | 📋 Phase 2 | rustls dangerous_configuration |
| Worker 部署 | 📋 Phase 2 | 模块化拆分 core/recovery/sync |
| 前端 | 📋 Phase 2+ | Vite 8 React SPA (USB 模块) |
| compute 模块 | 📋 已剥离 | 通过 Worker 调度或独立 daemon |

### 已踩过的坑

```
1. main 分支不要乱推 — 之前误推 Phase 1 到 main，花了很多时间恢复
2. nftables 实验先用双 SSH 窗口 — 断了就真连不回来了
3. /tmp 文件会被清理 — 代理脚本放 /tmp 只能用一个 turn
4. 记住 proxy_on — 以太网只能走 SOCKS5，WiFi 没网
5. pre-push 是唯一的防线 — 别绕过它，除非你知道自己在干什么
```

---

## 四、维护工作流

### 日常开发

```bash
# 1. 选分支
git checkout phase1.2

# 2. 改代码

# 3. 测试
cargo test

# 4. 提交
git add -A
git commit -m "type(scope): description"

# 5. 推送
source proxy-method.sh
git-proxy-push phase1.2
```

### 提交信息规范

```
格式: type(scope): 简短描述

type:
  feat    — 新功能
  fix     — 修 bug
  docs    — 文档
  refactor— 重构
  chore   — 杂项 (gitignore, 清理)
  perf    — 性能优化
  security— 安全修复

scope:
  core    — 核心认证/权限
  recovery— 恢复系统
  nftables— 防火墙
  deploy  — 部署/安装
  docs    — 文档

示例:
  feat(core): 添加短恢复码生成
  fix(recovery): HKDF 密钥派生漏洞
  docs: 更新用户手册
  chore: 清理无用脚本
```

### 定期维护

```
每小时:  GC 自动运行（后台任务）
每次实验后: 清理路由器临时文件
每次提交前:  cargo test
每次推送前:  检查敏感文件（pre-push 自动）
每周:  检查 event_log 大小
每月:  审查 pre-push 规则是否过时
```

---

## 五、关键人员/设备信息

| 项目 | 值 | 位置 |
|------|-----|------|
| 仓库 | ArchivalEra/Worker-Router-Gateway | GitHub |
| 路由器 | root@<router-ip> | 本地网络 |
| 路由器固件 | ImmortalWrt 6.18.37 / MT7621 | — |
| 开发机 | Debian sid, Rust 1.96, nightly | 本容器 |
| 代理 | socks5h://127.0.0.1:2080 | 以太网出口 |
| 域名 | ca.example.com | Cloudflare |
| Worker | Cloudflare Workers (待部署) | CF 面板 |
