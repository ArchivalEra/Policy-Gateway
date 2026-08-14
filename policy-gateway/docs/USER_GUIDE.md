# user-guide

`policy-gateway` — cert-controlled gateway. No cert, no internet.

## Two roles

| Role | What they can do |
|------|------------------|
| **Root admin** | `init` first setup, approve/reject, manage everything |
| **Device** | signup, wait for approval, use internet |

## Quickstart (root admin on router)

```bash
# First time setup
policy-gateway init          # generate CA + token + seed.json
policy-gateway init --confirm  # verify recovery works

# Start the server
policy-gateway serve

# Open browser
# http://<router-ip>:8443/manager?token=<token>
```

## Device signup

### Browser

1. Go to `http://<router-ip>:8443/signup`
2. Click "生成密钥对", fill in device name, submit
3. Wait for admin to approve (refresh /signup/status)
4. Once approved → internet access granted

### CLI (any device with HTTP)

```bash
export PG_SERVER=http://gateway:8443
export PG_TOKEN=<admin-token>

# Submit pubkey
pg cert sign "abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234"

# Check status
pg cert status <sha256>
```

### MCU (ESP32 / STM32)

1. Generate Ed25519 keypair, send pubkey hex to `/api/signup`
2. Connect to `/api/events` SSE to get real-time approval notification
3. No polling, no timers — pure event-driven

```
POST /api/signup → pending_confirm
GET  /api/events  ← SSE: {"type":"approved","sha256":"..."} (real-time)
```

## Recovery (root cert lost)

| Method | Dependencies |
|--------|-------------|
| Short code | Local only (SHA256 stored on router) |
| Cert-based | Local only (AES-256-GCM from cert key) |
| Worker | Cloudflare Pages (vm-worker) |

## Maintenance mode

```bash
policy-gateway-vm maintenance set <start_ts> <end_ts>
policy-gateway-vm maintenance status
policy-gateway-vm maintenance clear
```

## Configuration

All config in `~/.policy-gateway/config.toml`:

```toml
server_url = "http://localhost:8443"
manager_token_hash = "sha256hex..."
listen_port = 8443
enable_html = true
storage_backend = "redb"
worker_url = "https://your-worker.pages.dev"
worker_token = "xxx"
monitor_interfaces = ["br-lan"]
```

Env overrides: `PG_MANAGER_TOKEN`, `PG_WORKER_URL`, `PG_GATEWAY_SYNC_TOKEN`, `PG_STORAGE`, etc.
