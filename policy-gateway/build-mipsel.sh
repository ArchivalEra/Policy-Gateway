#!/bin/sh
# 交叉编译 policy-gateway 到 mipsel-unknown-linux-musl
#
# 环境: ImmortalWrt SDK 或 zig 工具链
# 作者建议: 在 3900X 上开 full LTO 编译

set -e
TARGET=mipsel-unknown-linux-musl
PROFILE=release

echo "=== 交叉编译 policy-gateway for $TARGET ==="

# 方式 1: cargo-zigbuild (推荐，zig 内置 mipsel musl 支持)
#   cargo install cargo-zigbuild
#   cargo zigbuild --target $TARGET --release

# 方式 2: ImmortalWrt SDK
#   下载 SDK: https://mirrors.nju.edu.cn/immortalwrt/releases/23.05.4/targets/ramips/mt7621/
#   export PATH=/path/to/sdk/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl/bin:$PATH
#   export CC_mipsel_unknown_linux_musl=mipsel-openwrt-linux-gcc
#   export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-openwrt-linux-gcc
#   cargo build --target $TARGET --release

# 方式 3: musl.cc 工具链
#   下载: wget https://musl.cc/mipsel-linux-musl-cross.tgz
#   tar xf mipsel-linux-musl-cross.tgz
#   export PATH=$PWD/mipsel-linux-musl-cross/bin:$PATH
#   cargo build --target $TARGET --release

echo ""
echo "选择一种方式，取消注释上面的对应段落，然后运行此脚本。"
echo "编译完成后用 UPX 压缩: upx --best target/$TARGET/$PROFILE/policy-gateway"
