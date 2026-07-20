#!/bin/sh
# 交叉编译 policy-gateway 到 mipsel-unknown-linux-musl
#
# 要求: ImmortalWrt SDK 或 musl 交叉编译器

set -e

TARGET=mipsel-unknown-linux-musl

echo "=== 交叉编译 policy-gateway for $TARGET ==="

# 如果使用 ImmortalWrt SDK，设置 PATH 指向工具链
# export PATH=/path/to/openwrt-sdk/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl/bin:$PATH
# export CC_mipsel_unknown_linux_musl=mipsel-openwrt-linux-gcc
# export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-openwrt-linux-gcc

# 或者使用系统 musl 交叉编译器:
# sudo apt install gcc-mipsel-linux-musl
# rustup target add mipsel-unknown-linux-musl

cargo build --target "$TARGET" --release

echo "=== 编译完成 ==="
ls -lh "target/$TARGET/release/policy-gateway"

# UPX 压缩
if command -v upx >/dev/null 2>&1; then
    echo "=== UPX 压缩 ==="
    upx --best "target/$TARGET/release/policy-gateway"
    ls -lh "target/$TARGET/release/policy-gateway"
fi

echo "=== 完成 ==="
