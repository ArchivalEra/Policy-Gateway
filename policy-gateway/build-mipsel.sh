#!/bin/sh
# build-mipsel.sh — 交叉编译 policy-gateway 到 mipsel-unknown-linux-musl
#
# 先运行 setup-cross.sh 下载工具链，然后运行本脚本：
#   ./setup-cross.sh
#   ./build-mipsel.sh
#
# 本脚本使用项目目录内的本地工具链，不依赖系统安装。

set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
export PATH="$HERE/zig-install:$HERE/mipsel-linux-musl-cross/bin:$PATH"

echo "=== 交叉编译 policy-gateway for mipsel-unknown-linux-musl ==="

# 优先用 cargo-zigbuild（zig 内置 mipsel musl 支持）
if command -v cargo-zigbuild 2>/dev/null; then
    cargo zigbuild --target mipsel-unknown-linux-musl --release
else
    # 退而用 musl-gcc + nightly build-std
    export CC_mipsel_unknown_linux_musl=mipsel-linux-musl-gcc
    export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-linux-musl-gcc
    cargo +nightly build -Z build-std --target mipsel-unknown-linux-musl --release
fi

echo "=== 编译完成 ==="
ls -lh "target/mipsel-unknown-linux-musl/release/policy-gateway" 2>/dev/null || \
ls -lh "target/release/policy-gateway" 2>/dev/null

# UPX 压缩
if command -v upx >/dev/null 2>&1; then
    echo "=== UPX 压缩 ==="
    BIN=$(find target -name policy-gateway -type f | head -1)
    [ -n "$BIN" ] && upx --best "$BIN" && ls -lh "$BIN"
fi
echo "=== 完成 ==="
