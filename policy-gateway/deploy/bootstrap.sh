#!/bin/sh
# bootstrap.sh — 首次引导：生成根证书 + 预置权限 + 安装服务
#
# 先跑这个，再 install.sh。确保你不会被锁在外面。
#
# 注意: 推荐使用 policy-gateway init 命令（更简洁，集成在程序中）
#   policy-gateway init
#   或通过 vm install 安装后运行: policy-gateway init
#
# bootstrap.sh 适用于没有 Rust 编译环境的场景。
#
# 用法:
#   ./bootstrap.sh                    # 生成 root cert + 预配置文件
#   ./bootstrap.sh --install          # 上述 + SCP 到路由器 + 安装
#
# 输出:
#   deploy/root-cert.pem    根证书（导入浏览器/系统）
#   deploy/root-key.pem     根私钥（妥善保管）
#   deploy/seed.json        预置权限表

set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

echo "============================================"
echo "  policy-gateway 首次引导"
echo "============================================"
echo ""

# --- Step 1: Generate root certificate ---
echo "=== 1. 生成根证书（10年有效期）==="
if [ ! -f deploy/root-key.pem ]; then
    openssl req -x509 -newkey ed25519 \
        -keyout deploy/root-key.pem -out deploy/root-cert.pem \
        -days 3650 -nodes \
        -subj "/CN=policy-gateway-root/O=policy-gateway/OU=admin"
    echo "   ✅ 根证书已生成"
else
    echo "   ⚠️  根证书已存在，跳过"
fi

ROOT_SHA256=$(openssl x509 -in deploy/root-cert.pem -noout -fingerprint -sha256 | \
    cut -d= -f2 | tr -d ':' | tr 'A-Z' 'a-z')

echo "   证书 SHA256: $ROOT_SHA256"
echo "   有效期: $(openssl x509 -in deploy/root-cert.pem -noout -dates | grep notBefore | cut -d= -f2) ~ $(openssl x509 -in deploy/root-cert.pem -noout -dates | grep notAfter | cut -d= -f2)"

# --- Step 2: Create pre-seeded permission table ---
echo ""
echo "=== 2. 预置权限表 ==="
cat > deploy/seed.json <<SEED
{
  "version": 1,
  "root_cert_sha256": "$ROOT_SHA256",
  "entries": [
    {
      "sha256": "$ROOT_SHA256",
      "hostname": "根管理员",
      "bitmap": 255,
      "status": "active",
      "mac": null,
      "created_at": $(date +%s)
    }
  ]
}
SEED
echo "   ✅ 权限表已创建: deploy/seed.json"

# --- Step 3: Generate manager token ---
echo ""
echo "=== 3. 管理令牌 ==="
if [ ! -f deploy/manager-token.txt ]; then
    TOKEN=$(openssl rand -hex 16)
    echo -n "$TOKEN" > deploy/manager-token.txt
    echo "   ✅ 管理令牌已生成"
else
    TOKEN=$(cat deploy/manager-token.txt)
    echo "   ⚠️  使用已有令牌"
fi
echo "   MANAGER_TOKEN=$TOKEN"

# --- Step 4: Create config ---
echo ""
echo "=== 4. 配置文件 ==="
cat > deploy/policy-gateway.conf <<CONF
config main
    option port '8443'
    option manager_token '$TOKEN'
    option log_level 'info'
    option vm_enabled '1'
    option seed_file '/etc/config/policy-gateway.seed.json'
CONF
echo "   ✅ 配置已创建: deploy/policy-gateway.conf"

# --- Step 5: Instructions ---
echo ""
echo "============================================"
echo "  引导完成！接下来："
echo ""
echo "  1. 把根证书导入你的设备："
echo "     deploy/root-cert.pem"
echo ""
echo "  2. 安装到路由器："
echo "     scp -P 22 deploy/root-cert.pem root@192.168.1.1:/etc/ssl/"
echo "     scp -P 22 deploy/seed.json root@192.168.1.1:/etc/config/policy-gateway.seed.json"
echo ""
echo "  3. 复制配置和安装："
echo "     scp -P 22 deploy/policy-gateway.conf root@192.168.1.1:/etc/config/policy-gateway"
echo "     scp -P 22 target/mipsel/release/policy-gateway root@192.168.1.1:/usr/sbin/"
echo "     ssh -p 22 root@192.168.1.1 '/etc/init.d/policy-gateway start'"
echo ""
echo "  4. 浏览器打开："
echo "     http://<router-ip>:8443/manager?token=$TOKEN"
echo "     导入根证书后即可登录"
echo ""
echo "  管理令牌: $TOKEN"
echo "  根证书:   deploy/root-cert.pem"
echo "============================================"