# 更新 GitHub 首页 README 方法

GitHub 仓库首页显示的 README 来源层级：

```
1. .github/README.md  ← 优先显示（仓库首页 + 组织首页）
2. README.md           ← 后备（.github/ 不存在时）
3. README.zh.md        ← 语言切换
```

## 更新步骤

```bash
# 1. 编辑根目录 README.md（英文）
vim README.md

# 2. 编辑中文版
vim README.zh.md

# 3. 同步 .github/README.md
cp README.md .github/README.md

# 4. 提交推送
git add -A
git commit -m "docs: 更新首页 README"
git push origin main
```

## 注意事项

- `.github/README.md` 必须和 `README.md` 内容一致，否则首页显示旧版
- GitHub 页面有 CDN 缓存，push 后约 1-5 分钟生效
- 缓存未刷新时可以通过 `?cachebuster=timestamp` 参数强制刷新
- 如果改仓库名，GitHub 自动 301 跳转，README 里的链接不需要手动更新
