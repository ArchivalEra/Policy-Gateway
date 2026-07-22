# 交叉编译指南（Agent 沙盒版）

> 目标: 在 Reasonix 容器内为 ImmortalWrt (mipsel_24kc) 交叉编译 Rust 二进制。
> 代价: 每次失败 ≈ 1 元。按本文档操作可一次成功。

## 前提

容器内的 `.toolchain/` 目录已包含:
- ImmortalWrt SDK (含 mipsel GCC 12.3.0)
- rust-src 1.96.0
- Zig 0.14.0

## 步骤

### 1. 建立可写 Rust 工具链

```bash
TOOLDIR="policy-gateway/.toolchain"
SYS_TC="/home/archivalera/.rustup/toolchains/stable-x86_64-unknown-linux-gnu"
MY_TC="$TOOLDIR/rustup-home-N/toolchains/stable-x86_64-unknown-linux-gnu"

mkdir -p "$MY_TC/bin" "$MY_TC/lib"
# 链接二进制
ln -sfn "$SYS_TC/bin/rustc" "$MY_TC/bin/rustc"
ln -sfn "$SYS_TC/bin/cargo" "$MY_TC/bin/cargo"
# 链接 LLVM 和运行时库（必须包括 .so.22.1 变体）
for f in "$SYS_TC/lib/libLLVM"* "$SYS_TC/lib/libstd-"*; do
  ln -sfn "$f" "$MY_TC/lib/"
done
# 链接 rustlib
ln -sfn "$SYS_TC/lib/rustlib" "$MY_TC/lib/rustlib"
# 替换 src/rust 为可写副本
rm -rf "$MY_TC/lib/rustlib/src/rust"
cp -r "$TOOLDIR/rust-src-1.96.0/rust-src/lib/rustlib/src/rust" \
  "$MY_TC/lib/rustlib/src/"

# rustup 配置
cat > "$TOOLDIR/rustup-home-N/settings.toml" << 'EOF'
default_toolchain = "stable-x86_64-unknown-linux-gnu"
version = "12"
EOF
```

### 2. 设置环境

```bash
export RUSTUP_HOME="$TOOLDIR/rustup-home-N"
export LD_LIBRARY_PATH="$MY_TC/lib"
export RUSTC_BOOTSTRAP=1

# SDK 工具链路径
SDK_DIR=$(ls -d "$TOOLDIR"/immortalwrt-sdk-*/staging_dir/toolchain-*/bin | head -1)
export PATH="$SDK_DIR:$PATH"
export CC_mipsel_unknown_linux_musl=mipsel-openwrt-linux-gcc
export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-openwrt-linux-gcc
```

### 3. 编译

```bash
cd policy-gateway
CARGO_TARGET_DIR="/tmp/pg-mips" \
  cargo build --target mipsel-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort
```

### 4. 部署到路由器

```bash
scp -P 22 target/mipsel-unknown-linux-musl/release/policy-gateway \
  root@192.168.1.1:/tmp/
ssh -p 22 root@192.168.1.1 "policy-gateway-vm install /tmp/policy-gateway"
```

## 常见失败原因

| 症状 | 原因 | 修复 |
|------|------|------|
| `rustc -vV` 失败 (libLLVM not found) | `libLLVM.so.22.1` 没 symlink | `for f in "$SYS_TC/lib/libLLVM"*; do ln -sfn "$f" "$MY_TC/lib/"; done` |
| `can't find crate for std` | rust-src 未正确安装 | 检查 `$MY_TC/lib/rustlib/src/rust/library/Cargo.lock` 存在 |
| `rustup could not choose a version` | settings.toml 缺失或格式错误 | 检查 version 字段："12" |
| `mipsel-openwrt-linux-gcc: not found` | SDK 路径不在 PATH | `export PATH="$SDK_DIR:\$PATH"` |
| `can't find crate for panic_abort` | build-std 参数不全 | 使用 `-Z build-std=core,alloc,std,panic_abort` |

## 验证

```bash
file /tmp/pg-mips/mipsel-unknown-linux-musl/release/policy-gateway-vm
# 输出: ELF 32-bit LSB, MIPS, MIPS32 rel2
```
