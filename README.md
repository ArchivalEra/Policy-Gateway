# 🔐 policy-gateway

**CA-certificate-controlled internet gateway for OpenWrt/ImmortalWrt routers.**

No certificate? No internet. Devices must submit a CSR (or public key), get it signed by the router's Ed25519 CA, and confirm in two phases before the gateway allows traffic.

[![CI](https://github.com/ArchivalEra/Policy-Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/ArchivalEra/Policy-Gateway/actions/workflows/ci.yml)
[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](https://github.com/ArchivalEra/Policy-Gateway/blob/main/LICENSE)
![Rust](https://img.shields.io/badge/rust-1.96+-orange)
![MIPS](https://img.shields.io/badge/target-mipsel--24kc-blueviolet)
![Platform](https://img.shields.io/badge/platform-ImmortalWrt%2FOpenWrt-orange)

[📖 中文版](https://github.com/ArchivalEra/Policy-Gateway/blob/main/README.zh.md)

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

See [`docs/CROSS_COMPILE.md`](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/CROSS_COMPILE.md) for detailed instructions.

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
| Router | MIPS (mipsel/mips), ARM (aarch64/armv7), x86_64, RISC-V |
| Flash | 2 MB (binary) + 100 KB (data) |
| RAM | 4 MB (runtime) |
| Kernel | Linux 5.15+ with nftables |
| Toolchain | Rust 1.96+, zig (for cross-compile) |
| OS | ImmortalWrt 23.05+, OpenWrt 22.03+ |

## Documentation

| English | 中文 |
|---------|------|
| [Architecture](https://github.com/ArchivalEra/Policy-Gateway/blob/main/PLAN.md) | [架构文档](https://github.com/ArchivalEra/Policy-Gateway/blob/main/PLAN.md) |
| [Building & Testing](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/README.md) | [编译指南](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/README.md) |
| [User Guide](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/USER_GUIDE.md) | [用户手册](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/USER_GUIDE.md) |
| [Cross-compilation Guide](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/CROSS_COMPILE.md) | [交叉编译](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/CROSS_COMPILE.md) |
| [Maintenance](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/MAINTENANCE.md) | [维护规章](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/MAINTENANCE.md) |
| [Module Guide](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/MODULE_GUIDE.md) | [模块指南](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/MODULE_GUIDE.md) |
| [Network diagnostics](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/NETWORK.md) | [网络诊断](https://github.com/ArchivalEra/Policy-Gateway/blob/main/policy-gateway/docs/NETWORK.md) |
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

**Active development — Phase 3.7** (v0.3.6)

| Phase | Highlights |
|-------|-----------|
| 0.5 | Core portal + permission table + GC |
| 2.0 | Two-phase confirm + hardware ID |
| 2.7 | CLI + EventLog + frontend feature gate |
| 2.9 | API-first restructure, CLI skeleton |
| 3.0 | Unified config, rclone-style CLI |
| 3.1 | i18n zh/en, QUIC module notice |
| 3.2 | TLS framework, nftables autodeploy, procd |
| 3.3 | Router-deployed v0.3.3, storage abstraction |
| 3.4 | storage-more module, bit claim, core freeze |
| 3.5 | Token hashing, VM hot-update, maintenance mode |
| 3.6 | worker-sync, perms auth, /api/help 4-guide, nftables security fix |
| **3.7** 🎯 | **仓库大扫除, lang-en feature, HTML 美化, SSE 事件驱动** |

- ✅ Core loop: signup → approve → active
- ✅ Ed25519 CA with two-phase commit
- ✅ nftables auto-deploy + cleanup
- ✅ procd init script for ImmortalWrt
- ✅ CLI client (zero-dependency)
- ✅ i18n: Chinese + English
- ✅ Cross-compilation for MIPS (MT7621 verified)
- ✅ Storage backend abstraction (redb / S3 / mirror stub)
- 🔄 TLS listener and plug-in modules (Phase 3.7+)

## License

[AGPL-3.0](https://github.com/ArchivalEra/Policy-Gateway/blob/main/LICENSE) — Free to use, modify, and distribute. Contributions welcome.
