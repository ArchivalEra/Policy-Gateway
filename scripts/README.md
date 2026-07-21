# scripts — 实用工具

| 脚本 | 说明 |
|------|------|
| `ssh_auto.py` | SSH 自动登录（pty + 密码） |
| `scp_auto.py` | SCP 自动上传 |
| `proxy-method.sh` | SOCKS5 代理 + git 推送方法（source 加载） |

使用代理推送:
```bash
source scripts/proxy-method.sh
git-proxy-push phase2.7-3
```
