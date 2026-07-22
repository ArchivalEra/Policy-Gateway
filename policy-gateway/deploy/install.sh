#!/bin/sh
# Install policy-gateway on ImmortalWrt/OpenWrt
#
# Usage:
#   ./install.sh              # interactive install
#   ./install.sh --minimize   # flash-optimized install
#   ./install.sh --modelize   # USB full install
#
# This script:
#   1. Copies the binary to /usr/sbin/
#   2. Sets up init script (/etc/init.d/policy-gateway)
#   3. Creates config directory (/etc/config/policy-gateway)
#   4. Optionally installs VM binary + watchdog
#   5. Optionally configures nftables captive portal

set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[✓]${NC} $1"; }
warn()  { echo -e "${YELLOW}[!]${NC} $1"; }
error() { echo -e "${RED}[✗]${NC} $1"; }

# --- Paths (follow OpenWrt standards) ---
BIN_DIR="/usr/sbin"
CONFIG_DIR="/etc/config"
DATA_DIR="/usr/share/policy-gateway"
MODULE_DIR="/usr/lib/policy-gateway/modules"
INIT_DIR="/etc/init.d"
WATCHDOG_FILE="/tmp/.policy-gateway-running"

# --- Detect architecture ---
ARCH=$(uname -m)
case "$ARCH" in
    mips|mipsel) BIN_SUFFIX="mipsel-musl" ;;
    armv7l|armv8l|aarch64) BIN_SUFFIX="arm-musl" ;;
    x86_64) BIN_SUFFIX="x86_64" ;;
    *) warn "未知架构: $ARCH，尝试默认"; BIN_SUFFIX="mipsel-musl" ;;
esac

echo "========================================"
echo "  policy-gateway — ImmortalWrt Install"
echo "  架构: $ARCH"
echo "========================================"
echo ""

# --- Prerequisites ---
echo "=== 检查依赖 ==="
for cmd in uci nft; do
    if ! command -v $cmd >/dev/null 2>&1; then
        error "$cmd 未安装"
        exit 1
    fi
done
info "依赖检查通过"

# --- Install binary ---
echo ""
echo "=== 安装主程序 ==="
cp policy-gateway "$BIN_DIR/policy-gateway" 2>/dev/null || error "未找到 policy-gateway 二进制"
chmod 755 "$BIN_DIR/policy-gateway"
info "安装到 $BIN_DIR/policy-gateway"

# --- Install VM ---
if [ -f policy-gateway-vm ]; then
    cp policy-gateway-vm "$BIN_DIR/policy-gateway-vm"
    chmod 755 "$BIN_DIR/policy-gateway-vm"
    info "VM 安装到 $BIN_DIR/policy-gateway-vm"
fi

# --- Init script ---
echo ""
echo "=== init.d 服务 ==="
if [ -f "$SCRIPT_DIR/policy-gateway.init" ]; then
    cp "$SCRIPT_DIR/policy-gateway.init" "$INIT_DIR/policy-gateway"
else
    # 内嵌备用
    cat > "$INIT_DIR/policy-gateway" << 'INIT'
#!/bin/sh /etc/rc.common
USE_PROCD=1
START=95
STOP=10
NAME=policy-gateway
PROG=/usr/bin/policy-gateway
start_service() {
    procd_open_instance
    procd_set_param command "$PROG" serve
    procd_set_param env RUST_LOG info
    procd_set_param respawn
    procd_set_param stdout 1
    procd_set_param stderr 1
    procd_close_instance
}
stop_service() {
    nft delete table inet pg_pre 2>/dev/null || true
    nft delete table ip pg_nat 2>/dev/null || true
}
INIT
fi
chmod 755 "$INIT_DIR/policy-gateway"
/etc/init.d/policy-gateway enable 2>/dev/null || true
info "服务注册: $INIT_DIR/policy-gateway"

# --- Config ---
echo ""
echo "=== 配置文件 ==="
mkdir -p "$CONFIG_DIR" "$DATA_DIR" "$MODULE_DIR"
if [ ! -f "$CONFIG_DIR/policy-gateway" ]; then
    cat > "$CONFIG_DIR/policy-gateway" << 'CONF'
config main
    option port '8443'
    option manager_token ''
    option log_level 'info'
    option vm_enabled '1'

config module
    option name 'compute'
    option path '/usr/lib/policy-gateway/modules/compute.so'
    option enabled '0'

config module
    option name 'storage'
    option path '/usr/lib/policy-gateway/modules/storage.so'
    option enabled '0'
CONF
    info "默认配置已创建"
else
    warn "配置已存在，跳过"
fi

# --- UCI firewall integration (nftables captive portal) ---
echo ""
echo "=== nftables 门户规则 ==="
if ! nft list chain inet fw4 policy_gateway 2>/dev/null >/dev/null; then
    cat > /tmp/pg-nftables.conf << 'NFT'
table inet fw4
chain policy_gateway {
    type nat hook prerouting priority 0;
    policy accept;

    # 重定向 HTTP 到门户
    tcp dport 80 redirect to :8443
    # 重定向 HTTPS 到门户（自签证书）
    tcp dport 443 redirect to :8443
}
NFT
    nft -f /tmp/pg-nftables.conf 2>/dev/null && info "nftables 规则已添加" || warn "nftables 规则添加失败"
else
    warn "nftables 规则已存在"
fi

echo ""
echo "========================================"
echo -e "  ${GREEN}安装完成!${NC}"
echo ""
echo "  启动:  /etc/init.d/policy-gateway start"
echo "  日志:  logread -e policy-gateway"
echo "  管理:  http://192.168.1.1:8443/manager"
echo ""
echo "  配置:  $CONFIG_DIR/policy-gateway"
echo "  二进制: $BIN_DIR/policy-gateway"
echo "  VM:    $BIN_DIR/policy-gateway-vm"
echo "========================================"
