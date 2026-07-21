#!/bin/sh
# build.sh — 编译 policy-gateway 全组件
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

echo "=== Building policy-gateway ==="
cargo build --release

echo ""
echo "=== Building policy-gateway-vm ==="
cargo build -p policy-gateway-vm --release

echo ""
echo "=== Binaries ==="
ls -lh target/release/policy-gateway target/release/policy-gateway-vm

echo ""
echo "=== To cross-compile for mipsel: ==="
echo "cargo zigbuild --target mipsel-unknown-linux-musl --release"
echo "See docs/CROSS_COMPILE.md"
