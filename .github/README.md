# CI/CD — GitHub Actions

## 自动运行

推送到任何分支时自动运行:

1. **cargo test** — 全量单元测试
2. **cargo build --release** — 发布编译  
3. **mipsel 交叉编译** — 验证路由器架构可编译

## 配置

不需要额外 token。GitHub 自动提供 `GITHUB_TOKEN`。
交叉编译使用 `cargo-zigbuild` + zig（自动下载）。

## 本地模拟

```bash
act -j test
act -j mipsel-cross
```
