# 🔐 policy-gateway

**CA-certificate-controlled internet gateway for OpenWrt/ImmortalWrt routers.**

No certificate? No internet. Devices must submit a CSR (or public key), get it signed by the router's Ed25519 CA, and confirm in two phases before the gateway allows traffic.

[![CI](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml)
[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.96+-orange)
![MIPS](https://img.shields.io/badge/target-mipsel--24kc-blueviolet)
![Platform](https://img.shields.io/badge/platform-ImmortalWrt%2FOpenWrt-orange)

[📖 中文版](README.zh.md)

```
Device          Router (policy-gateway)
  │                   │
  ├─ CSR/pubkey ─────▶│  CA sign (Ed25519)
  │◀──── pending_confirm ─┤
  ├─ confirm ─────────▶│  activate
  │◀──── internet ───────┤
```

## Features

- **Ed25519 CA** — signs certificates on the router with its own private key

---

*This project was born from a ¥45 newifi3 D2 (MT7621AT, 128MB RAM, 10MB SPI flash) bought on a second-hand market. Every design decision — single binary, no dynamic linking, bitmap permissions instead of certificate chains, nftables instead of iptables, zero-copy event log, sub-1MB UPX target — was forced by that hardware. If it runs on a decade-old MIPS router with 10MB of flash, it runs anywhere.*

---
- **Two-phase commit** — prevents ghost certificates on network drop
- **Bitmap permissions** — 64-bit bitmap for fine-grained control (connector/admin/device/storage/compute)
- **No mTLS overhead** — SHA256 whitelist lookup (O(1)), no certificate chain verification at runtime
- **Captive portal** — nftables redirects unauthorized devices to `/signup`
- **CLI client** — zero-dependency (`pg` binary, raw TCP)
- **Bilingual** — Chinese / English UI and documentation
- **VM (不死鸟)** — standalone binary for snapshot/rollback of the main program
- **Recovery** — Cloudflare Pages Worker for root certificate recovery (/recover)

## Quick start

```bash
git clone https://github.com/ArchivalEra/policy-gateway.git
cd policy-gateway

# Build & test
cargo test                          # 41 tests, 0 warnings
cargo build --release               # ~1.7MB stripped

# Run locally
MANAGER_TOKEN=test ./target/release/policy-gateway serve
```

Open `http://localhost:8443/manager?token=test`

### Deploy on router (MIPS cross-compile)

```bash
# Cross-compile
rustup target add mipsel-unknown-linux-musl
cargo build --target mipsel-unknown-linux-musl --release -Z build-std

# SCP to router
scp target/mipsel-unknown-linux-musl/release/policy-gateway root@<router-ip>:/tmp/
ssh root@<router-ip> "MANAGER_TOKEN=<token> /tmp/policy-gateway serve &"
```

See [`docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md) for detailed instructions.

## Architecture

```
┌─────────────────────────────────────────────────┐
│  policy-gateway (Rust, event-driven)             │
│                                                   │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ CA Engine │  │ Auth Table   │  │ EventLog   │ │
│  │ Ed25519   │  │ Bitmap(u64)  │  │ Append-only│ │
│  │ sign_csr  │  │ GC rules     │  │ Timestamps │ │
│  └──────────┘  └──────────────┘  └────────────┘ │
│                                                   │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ API      │  │ nftables     │  │ CLI (pg)   │ │
│  │ signup   │  │ pg_pre/pg_nat│  │ approve    │ │
│  │ manager  │  │ REDIRECT 80  │  │ pending    │ │
│  │ confirm  │  │ authorized   │  │ status     │ │
│  └──────────┘  └──────────────┘  └────────────┘ │
│                                                   │
│  ┌──────────┐  ┌──────────────┐  ┌────────────┐ │
│  │ VM (独)  │  │ Worker       │  │ Storage    │ │
│  │ Snapshot │  │ Recovery     │  │ redb/S3    │ │
│  │ Rollback │  │ /recover     │  │ mirror     │ │
│  └──────────┘  └──────────────┘  └────────────┘ │
└─────────────────────────────────────────────────┘
          │                              ▲
          │ HTTP/TLS (8443)              │ nftables REDIRECT 80/443
          ▼                              │
     ┌──────────┐             ┌──────────────────┐
     │ Browser  │             │ Unauthorized dev │
     │ /signup  │             │ → /signup portal │
     │ /manager │             └──────────────────┘
     └──────────┘
```

## Requirements

| Item | Minimum |
|------|---------|
| Router | MT7620/MT7621 (MIPS 24kc), x86_64, ARM |
| Flash | 2 MB (binary) + 100 KB (data) |
| RAM | 4 MB (runtime) |
| Kernel | Linux 5.15+ with nftables |
| Toolchain | Rust 1.96+, zig (for cross-compile) |
| OS | ImmortalWrt 23.05+, OpenWrt 22.03+ |

## Documentation

| English | 中文 |
|---------|------|
| [Architecture](PLAN.md) | [核心架构](PLAN.md) (中英混) |
| [Building & Testing](policy-gateway/README.md) | [编译指南](policy-gateway/README.md) |
| [User Guide](policy-gateway/docs/USER_GUIDE.md) | [用户手册](policy-gateway/docs/USER_GUIDE.md) |
| [Cross-compilation Guide](policy-gateway/docs/CROSS_COMPILE.md) | [交叉编译](policy-gateway/docs/CROSS_COMPILE.md) |
| [Maintenance](policy-gateway/docs/MAINTENANCE.md) | [维护规章](policy-gateway/docs/MAINTENANCE.md) |
| [Module Guide](policy-gateway/docs/MODULE_GUIDE.md) | [模块指南](policy-gateway/docs/MODULE_GUIDE.md) |
| [Network diagnostics](policy-gateway/docs/NETWORK.md) | [网络诊断](policy-gateway/docs/NETWORK.md) |
| MCU tutorial: `/api/help?topic=mcu` | MCU 教程: `/api/help?topic=mcu` |

## CLI

```bash
policy-gateway serve              # Start web server
policy-gateway init               # First setup: CA + token + TLS cert
policy-gateway config edit        # Interactive configuration
policy-gateway config tls edit    # TLS cipher/profile config
policy-gateway perm gc            # GC expired entries
policy-gateway-vm snapshot test   # Create VM snapshot
policy-gateway-vm rollback test   # Rollback
```

## Project Status

**Active development — Phase 3.3** (v0.3.3)

| Phase | Highlights |
|-------|-----------|
| 0.5 | Core portal + permission table + GC |
| 2.0 | Two-phase confirm + hardware ID |
| 2.7 | CLI + EventLog + frontend feature gate |
| 2.9 | API-first restructure, CLI skeleton |
| 3.0 | Unified config, rclone-style CLI |
| 3.1 | i18n zh/en, QUIC module notice |
| 3.2 | TLS framework, nftables autodeploy, procd, vm-worker |
| **3.3** 🎯 | **Router-deployed v0.3.3, store panic fix, storage abstraction** |

- ✅ Core loop: signup → approve → active
- ✅ Ed25519 CA with two-phase commit
- ✅ nftables auto-deploy + cleanup
- ✅ procd init script for ImmortalWrt
- ✅ CLI client (zero-dependency)
- ✅ i18n: Chinese + English
- ✅ Cross-compilation for MIPS (MT7621 verified)
- ✅ Storage backend abstraction (redb / S3 / mirror stub)
- 🔄 TLS listener (Phase 3.4)
- 🔄 Plug-in modules (compute, quic)

## License

[AGPL-3.0](LICENSE) — Free to use, modify, and distribute. Contributions welcome.
