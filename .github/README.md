# 🔐 policy-gateway

**No certificate, no internet.** Router signs certs with Ed25519, permissions in a bitmap, two-phase confirm before allowing traffic.

[![CI](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/ArchivalEra/Worker-Router-Gateway/actions/workflows/ci.yml)
[![AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue)](LICENSE)
![Rust](https://img.shields.io/badge/rust-1.96+-orange)

[📖 中文版](README.zh.md)

```
Device → CSR → CA sign(Ed25519) → Client confirm → Internet access
```

## Quick start

```bash
git clone https://github.com/ArchivalEra/Worker-Router-Gateway.git
cd policy-gateway
cargo test    # 41 tests, 0 warnings
cargo build --release
MANAGER_TOKEN=test ./target/release/policy-gateway serve
```

Open `http://localhost:8443/manager?token=test`

## Architecture

```
┌────────────────────────────────────────────┐
│  Core (Rust, event-driven, zero unsafe)    │
│  CA engine     Permission tbl   Device ID  │
│  Ed25519       Bitmap(u64)     GC rules    │
│  sign_csr      Reuse detection  EventLog   │
├────────────────────────────────────────────┤
│  signup        confirm         manager     │
│  CSR/pubkey    Two-phase       Admin page  │
├────────────────────────────────────────────┤
│  CLI           VM              Worker      │
│  perm gc/list  Install/rollbk  Recovery    │
└────────────────────────────────────────────┘
```

## Components

| Component | Description |
|-----------|-------------|
| `policy-gateway` | Main binary: HTTP server + CA engine + CLI |
| `policy-gateway-vm` | Snapshot manager: install/rollback (standalone binary) |
| `worker/vm-worker` | Cloudflare Pages: minimal recovery (/recover) |

## Documentation

| | |
|------|------|
| Architecture | [`PLAN.md`](PLAN.md) |
| Build guide | [`policy-gateway/README.md`](policy-gateway/README.md) |
| User guide (zh) | [`policy-gateway/docs/USER_GUIDE.md`](policy-gateway/docs/USER_GUIDE.md) |
| Cross compile | [`policy-gateway/docs/CROSS_COMPILE.md`](policy-gateway/docs/CROSS_COMPILE.md) |
| Maintenance | [`policy-gateway/docs/MAINTENANCE.md`](policy-gateway/docs/MAINTENANCE.md) |
| Module guide | [`policy-gateway/docs/MODULE_GUIDE.md`](policy-gateway/docs/MODULE_GUIDE.md) |
| MCU tutorial | Visit `/api/help?topic=mcu` after starting |

## Deploy (OpenWrt / ImmortalWrt)

```bash
# 1. Install VM
scp policy-gateway-vm root@<router-ip>:/usr/sbin/
ssh root@<router-ip> "policy-gateway-vm init"

# 2. Install main binary (cross-compiled)
scp policy-gateway root@<router-ip>:/tmp/
ssh root@<router-ip> "policy-gateway-vm install /tmp/policy-gateway"

# 3. Start
ssh root@<router-ip> "MANAGER_TOKEN=<token> policy-gateway serve &"
```

## CLI

```bash
policy-gateway serve              # Start web server
policy-gateway init               # First setup (generate CA + token)
policy-gateway config edit        # Interactive config
policy-gateway perm gc            # GC expired entries
policy-gateway-vm snapshot test   # Create snapshot
policy-gateway-vm rollback test   # Rollback
```

## Status

| Phase | Content |
|-------|---------|
| 0.5 | Core portal + permission table + GC |
| 1 | Recovery + CLI + nftables integration |
| 1.1 | nftables dual-table |
| 1.5 | Maintenance manual + experiment rules |
| 1.5-2 | CA engine (generate_ca / sign_csr) |
| 2.0 | PendingConfirm + hw_id / hw_platform |
| 2.2 | device_id + security review + 0 warnings |
| 2.5 | Public release prep |
| 2.7 | CLI + EventLog + frontend gate |
| 2.8 | Worker Pages deploy + module guide |
| 2.9 | Restructure: API-first, CLI skeleton |
| 3.0 | Unified config + rclone-style CLI + TLS profile |
| 3.1 | i18n zh/en + QUIC notice |
| **3.2** 🔄 | **TLS framework + nftables autodeploy + procd + vm-worker merge** |
