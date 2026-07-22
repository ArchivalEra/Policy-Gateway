#!/bin/sh
# cross-deploy.sh — 在完整开发机上一键交叉编译 + 部署到路由器
# 用法: ./cross-deploy.sh                    # 编译 + SCP + 安装
# 用法: ./cross-deploy.sh --router-only      # 仅安装路由器端（需已有二进制）
#
# 环境: 需要 Home PC (3900X), 不需要本容器
# 前提: git clone 本项目, cd policy-gateway

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

ROUTER="root@192.168.1.1"
PORT="22"
KEY="$SCRIPT_DIR/../newifi3_key"

echo "=== 政策网关 交叉部署 ==="
echo ""

# 1. 交叉编译
echo ">>> 1/4 交叉编译..."
TOOLDIR="$SCRIPT_DIR/.toolchain"
SDK_DIR=$(ls -d "$TOOLDIR"/immortalwrt-sdk-*/staging_dir/toolchain-*/bin 2>/dev/null | head -1)

if [ -z "$SDK_DIR" ]; then
    echo "  SDK 未找到，使用 zig 编译..."
    cargo zigbuild --target mipsel-unknown-linux-musl --release
    BIN="target/mipsel-unknown-linux-musl/release/policy-gateway"
else
    echo "  使用 ImmortalWrt SDK..."
    export PATH="$SDK_DIR:$PATH"
    export CC_mipsel_unknown_linux_musl=mipsel-openwrt-linux-gcc
    export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-openwrt-linux-gcc
    export RUSTC_BOOTSTRAP=1
    cargo build --target mipsel-unknown-linux-musl --release -Z build-std
    BIN="target/mipsel-unknown-linux-musl/release/policy-gateway"
fi

# 也编译 VM
echo "  VM 二进制..."
cargo build -p policy-gateway-vm --release
VM_BIN="target/release/policy-gateway-vm"

echo "  ✅ 编译完成: $BIN"

# 2. SCP 到路由器
echo ">>> 2/4 上传到路由器..."
ssh -p "$PORT" -i "$KEY" -o StrictHostKeyChecking=no "$ROUTER" "mkdir -p /tmp/pg-deploy"
scp -P "$PORT" -i "$KEY" -o StrictHostKeyChecking=no "$BIN" "$ROUTER:/tmp/pg-deploy/policy-gateway"
scp -P "$PORT" -i "$KEY" -o StrictHostKeyChecking=no "$VM_BIN" "$ROUTER:/tmp/pg-deploy/policy-gateway-vm"
echo "  ✅ 上传完成"

# 3. 安装 VM + 主程序
echo ">>> 3/4 安装..."
ssh -p "$PORT" -i "$KEY" "$ROUTER" "
    # 安装 VM
    cp /tmp/pg-deploy/policy-gateway-vm /usr/sbin/
    policy-gateway-vm init

    # 安装主程序
    policy-gateway-vm install /tmp/pg-deploy/policy-gateway

    # 创建配置
    mkdir -p /etc/config/policy-gateway
    cat > /etc/config/policy-gateway/seed.json << 'EOFSEED'
{
  \"manager_token\": \"$(uuidgen 2>/dev/null || echo 'deploy-token-change-me')\",
  \"entries\": []
}
EOFSEED

    # 部署 nftables 规则
    sh /etc/config/policy-gateway/pg-nftables.sh setup
"
echo "  ✅ 安装完成"

# 4. 启动服务
echo ">>> 4/4 启动服务..."
ssh -p "$PORT" -i "$KEY" "$ROUTER" "
    MANAGER_TOKEN=\$(cat /etc/config/policy-gateway/seed.json | grep manager_token | cut -d'\"' -f4)
    export MANAGER_TOKEN=\$MANAGER_TOKEN
    nohup /usr/sbin/policy-gateway > /var/log/policy-gateway.log 2>&1 &
    sleep 2
    echo \"  管理面板: http://192.168.1.1:8443/manager?token=\$MANAGER_TOKEN\"
"

echo ""
echo "=== 🎉 部署完成 ==="
echo "  浏览器打开: http://192.168.1.1:8443"
echo "  管理面板: http://<router-ip>:8443/manager?token=<seed.json 中的 token>"
