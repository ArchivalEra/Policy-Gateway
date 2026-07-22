# 文档同步维护工作流

每个新 Phase 发布时，按此清单逐项更新。

## 1. 版本号更新

```bash
# 更新三个地方
policy-gateway/Cargo.toml          # version = "x.y.z"
policy-gateway/vm/Cargo.toml       # version = "x.y.z"
git tag vx.y.z                     # 创建标签
git push origin vx.y.z             # 推送标签
```

## 2. README 双语更新

### 2.1 状态表

两个 README 的 Project Status / 项目状态 表必须同步：

```markdown
| **3.x** 🎯 | **新功能简述** |
```

英文版在 `README.md`，中文版在 `README.zh.md`。每次加一行。

### 2.2 Features / 特点

如果新增了功能特性，在 Features 列表末尾追加（双语）。
如果只是内部重构，不一定要加。

### 2.3 CLI 示例

新增子命令时，在 CLI 章节加入示例（双语）。

## 3. 文档横向同步

| 文件 | 更新什么 |
|------|---------|
| `PLAN.md` | 核心架构 → 在 "Phase X" 章节追加新阶段设计 |
| `USER_GUIDE.md` | 用户手册 → CLI 新命令、新配置项、新工作流 |
| `CROSS_COMPILE.md` | 交叉编译 → 工具链版本、新依赖 |
| `MAINTENANCE.md` | 维护规章 → 新模块、新实验规则 |
| `MODULE_GUIDE.md` | 模块指南 → 新增模块说明 |
| `NETWORK.md` | 网络诊断 → 端口变化、新协议 |

## 4. 检查项清单

```bash
# 每次发布前跑:
cd policy-gateway
cargo test                    # 41 tests, 0 warnings
cargo build --release         # 编译通过
cd ..
git status --short            # 无未跟踪敏感文件
grep -rn '/home/archivalera'  # 无个人路径泄漏
grep -rn '192\.168\.50\.1'    # 无硬编码内网 IP

# 检查版本一致性:
grep "^version" policy-gateway/Cargo.toml
grep "^version" policy-gateway/vm/Cargo.toml
git tag -l | sort -V | tail -1
```

## 5. 提交与推送

```bash
# 最终提交
git add -A
git commit -m "Phase X.Y: 简短描述"

# 推送到 main
SKIP_MAIN_CHECK=1 git push origin main --no-verify
git push origin vx.y.z
```
