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
# 代理: 如果 proxy_on 可用，会自动启用

set -e

if command -v proxy_on 2>/dev/null; then
    proxy_on 2>/dev/null
fi

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

download() {
    url="$1"
    out="$2"
    if [ -f "$out" ]; then
        echo "  ✅ 已有 $out"
        return 0
    fi
    echo "  ⬇️  下载 $url"
    curl -sL --connect-timeout 15 --max-time 600 "$url" -o "$out"
    ls -lh "$out"
}

setup_zig() {
    echo "=== Zig (用于 cargo-zigbuild) ==="
    if [ -x zig-install/zig ]; then
        echo "  ✅  zig 已安装: $(zig-install/zig version)"
        return
    fi
    download "https://ziglang.org/download/0.14.0/zig-linux-x86_64-0.14.0.tar.xz" /tmp/zig.tar.xz
    tar xf /tmp/zig.tar.xz -C "$HERE"
    mv "$HERE"/zig-linux-x86_64-* "$HERE"/zig-install
    echo "  ✅ zig $(zig-install/zig version) 就绪"
}

setup_musl() {
    echo "=== musl 交叉编译器 ==="
    if [ -x mipsel-linux-musl-cross/bin/mipsel-linux-musl-gcc ]; then
        echo "  ✅ musl-gcc 已安装"
        return
    fi
    download "https://musl.cc/mipsel-linux-musl-cross.tgz" /tmp/musl.tgz
    tar xzf /tmp/musl.tgz -C "$HERE"
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
