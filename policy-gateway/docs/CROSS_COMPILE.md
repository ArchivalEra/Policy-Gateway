# policy-gateway 交叉编译指南

支持所有 ImmortalWrt / OpenWrt 架构。

## 方法一: SDK 编译 (推荐)

从 ImmortalWrt 官网下载对应架构的 SDK。

### mipsel (MT7620/MT7621, newifi3 等)

```bash
# 下载 SDK
wget https://downloads.immortalwrt.org/snapshots/targets/ramips/mt7621/immortalwrt-sdk-23.05.4-ramips-mt7621_gcc-12.3.0_musl.Linux-x86_64.tar.xz
tar xf immortalwrt-sdk-*.tar.xz
export PATH="$PWD/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl/bin:$PATH"
export CC_mipsel_unknown_linux_musl=mipsel-openwrt-linux-gcc
export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-openwrt-linux-gcc
export RUSTC_BOOTSTRAP=1

cargo build --target mipsel-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort
```

### aarch64 (IPQ8074, MT7986 等)

```bash
# 下载 SDK (armvirt 或 ipq8074)
wget https://downloads.immortalwrt.org/snapshots/targets/armsr/armv8/immortalwrt-sdk-*-aarch64_gcc-*_musl.Linux-x86_64.tar.xz
export PATH="$PWD/staging_dir/toolchain-aarch64_*-gcc-*/bin:$PATH"
export CC_aarch64_unknown_linux_musl=aarch64-openwrt-linux-gcc
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-openwrt-linux-gcc
export RUSTC_BOOTSTRAP=1

cargo build --target aarch64-unknown-linux-musl --release \
  -Z build-std=core,alloc,std,panic_abort
```

### armv7 (32-bit ARM)

```bash
# SDK: target/ipq40xx 或 target/sunxi
export PATH="$PWD/staging_dir/toolchain-arm_cortex-a7_*-gcc-*/bin:$PATH"
export CC_armv7_unknown_linux_musleabihf=arm-openwrt-linux-gcc
export CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-openwrt-linux-gcc
export RUSTC_BOOTSTRAP=1

cargo build --target armv7-unknown-linux-musleabihf --release \
  -Z build-std=core,alloc,std,panic_abort
```

## 方法二: zigbuild (无需 SDK)

```bash
cargo install cargo-zigbuild
rustup component add rust-src

# 编译任意架构
cargo zigbuild --target mipsel-unknown-linux-musl --release -Z build-std
cargo zigbuild --target aarch64-unknown-linux-musl --release -Z build-std
cargo zigbuild --target x86_64-unknown-linux-musl --release -Z build-std
```

## 支持的架构

| Rust 目标 | ImmortalWrt 架构 | 常见设备 |
|-----------|-----------------|---------|
| `mipsel-unknown-linux-musl` | mipsel_24kc | MT7620, MT7621 (newifi3) |
| `mips-unknown-linux-musl` | mips_24kc | 某些 QCA 芯片 |
| `aarch64-unknown-linux-musl` | aarch64_cortex-a53/a72 | IPQ8074, MT7986, Raspberry Pi |
| `armv7-unknown-linux-musleabihf` | arm_cortex-a7 | IPQ40xx, MT7623 |
| `x86_64-unknown-linux-musl` | x86_64 | x86 软路由 |
| `riscv64gc-unknown-linux-musl` | riscv64 | 未来设备 |
