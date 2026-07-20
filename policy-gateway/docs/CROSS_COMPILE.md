# policy-gateway 交叉编译指南（mipsel-unknown-linux-musl）

目标设备: newifi3 (ImmortalWrt, mipsel_24kc, musl libc)
推荐主机: 3900X + 32GB RAM

## 方法一: cargo-zigbuild（推荐，最省心）

```bash
# 1. 安装 zig
wget https://ziglang.org/download/0.14.0/zig-linux-x86_64-0.14.0.tar.xz
tar xf zig-linux-x86_64-0.14.0.tar.xz
export PATH=$PWD/zig-linux-x86_64-0.14.0:$PATH

# 2. 安装 cargo-zigbuild
cargo install cargo-zigbuild

# 3. 编译
cd policy-gateway
cargo zigbuild --target mipsel-unknown-linux-musl --release

# 4. 压缩
upx --best target/mipsel-unknown-linux-musl/release/policy-gateway
```

## 方法二: musl.cc 交叉编译器 + nightly Rust

```bash
wget https://musl.cc/mipsel-linux-musl-cross.tgz
tar xzf mipsel-linux-musl-cross.tgz
export PATH=$PWD/mipsel-linux-musl-cross/bin:$PATH
rustup toolchain install nightly -c rust-src
cd policy-gateway
cargo +nightly build -Z build-std --target mipsel-unknown-linux-musl --release
upx --best target/mipsel-unknown-linux-musl/release/policy-gateway
```

## 方法三: ImmortalWrt SDK

```bash
wget https://mirrors.nju.edu.cn/immortalwrt/releases/23.05.4/targets/ramips/mt7621/immortalwrt-sdk-23.05.4-ramips-mt7621_gcc-12.3.0_musl.Linux-x86_64.tar.xz
tar xf immortalwrt-sdk-*.tar.xz
export PATH=$PWD/sdk/staging_dir/toolchain-mipsel_24kc_gcc-12.3.0_musl/bin:$PATH
export CARGO_TARGET_MIPSEL_UNKNOWN_LINUX_MUSL_LINKER=mipsel-openwrt-linux-gcc
cd policy-gateway && cargo build --target mipsel-unknown-linux-musl --release
```

## 编译优化

`Cargo.toml` 已配置 `lto=true, codegen-units=1, opt-level=z, strip=true, panic=abort`。
