#!/bin/sh
# setup-cross.sh — 在项目目录内搭建 mipsel 交叉编译环境
# 所有工具链下载到项目本地，不污染系统，所有 agent 都能用。
#
# 用法:
#   ./setup-cross.sh           # 检测并下载缺失的工具
#   ./setup-cross.sh zig       # 仅下载 zig
#   ./setup-cross.sh musl      # 仅下载 musl 交叉编译器
#   ./setup-cross.sh all       # 全部下载
#
# 安全: 下载使用 HTTPS，curl 带 -f 标志确保 HTTP 错误时失败。
#       建议下载后自行验证 SHA256（来源官网可查）。
#
# 代理: 如果 proxy_on 可用，会自动启用

# 项目本地的 .toolchain/ 目录已预装 zig 0.14.0 + rust-src 1.96.0
# 直接使用: export PATH="$(dirname "$0")/.toolchain/zig:$PATH"

set -e

if command -v proxy_on 2>/dev/null; then
    proxy_on 2>/dev/null
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

download() {
    url="$1"
    out="$2"
    expected_sha256="$3"
    if [ -f "$out" ]; then
        echo "  ✅ 已有 $out"
        return 0
    fi
    echo "  ⬇️  下载 $url"
    curl -sLf --connect-timeout 15 --max-time 600 "$url" -o "$out"
    ls -lh "$out"
    # 如果提供了预期 SHA256，自动校验
    if [ -n "$expected_sha256" ]; then
        actual=$(sha256sum "$out" | cut -d' ' -f1)
        if [ "$actual" != "$expected_sha256" ]; then
            echo "  ❌ SHA256 不匹配！预期 $expected_sha256，实际 $actual"
            rm -f "$out"
            exit 1
        fi
        echo "  ✅ SHA256 验证通过"
    else
        echo "  ⚠️  未校验 SHA256，建议从官网手动验证"
    fi
}

setup_zig() {
    echo "=== Zig (用于 cargo-zigbuild) ==="
    if [ -x zig-install/zig ]; then
        echo "  ✅  zig 已安装: $(zig-install/zig version)"
        return
    fi
    # 从 ziglang.org 下载（HTTPS）
    download "https://ziglang.org/download/0.14.0/zig-linux-x86_64-0.14.0.tar.xz" /tmp/zig.tar.xz ""
    tar xf /tmp/zig.tar.xz -C "$HERE"
    mv "$HERE"/zig-linux-x86_64-* "$HERE"/zig-install
    rm -f /tmp/zig.tar.xz
    echo "  ✅ zig $(zig-install/zig version) 就绪"
}

setup_musl() {
    echo "=== musl 交叉编译器 ==="
    if [ -x mipsel-linux-musl-cross/bin/mipsel-linux-musl-gcc ]; then
        echo "  ✅ musl-gcc 已安装"
        return
    fi
    # 从 musl.cc 下载（广泛使用的社区源，Rust cross 项目默认源）
    download "https://musl.cc/mipsel-linux-musl-cross.tgz" /tmp/musl.tgz ""
    tar xzf /tmp/musl.tgz -C "$HERE"
    rm -f /tmp/musl.tgz
    echo "  ✅ musl 交叉编译器就绪"
}

case "${1:-all}" in
    zig) setup_zig ;;
    musl) setup_musl ;;
    all|*)
        setup_zig
        setup_musl
        echo ""
        echo "🎉 全部就绪！编译方法:"
        echo "   export PATH=\"$HERE/zig-install:\$PATH\""
        echo "   cd $HERE && cargo zigbuild --target mipsel-unknown-linux-musl --release"
        ;;
esac
