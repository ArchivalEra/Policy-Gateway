# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in policy-gateway, please report it privately via GitHub's security advisory feature or by contacting the maintainers directly.

Please **do not** report security vulnerabilities through public GitHub issues.

## Supported Versions

| Version | Supported |
|---------|-----------|
| v0.2.x  | ✅ Current development |
| < 0.2   | ❌ |

## Security Features

- **Constant-time token comparison**: All authentication uses XOR-based constant-time comparison to prevent timing attacks.
- **No unsafe code**: policy-gateway is written in pure Rust with zero `unsafe` blocks.
- **Ed25519 signatures**: CA uses Ed25519 for signing, not RSA (faster and smaller keys).
- **SHA256 identity**: Certificates are identified by SHA256 hash, not by serial number or subject.
- **AES-256-GCM encryption**: Recovery certificates are encrypted with AES-256-GCM + HKDF-SHA256.
- **Minimal attack surface**: The binary is a single ~2MB static musl binary with no shell, no plugins, no dynamic loading.
- **VM separation**: The version manager (policy-gateway-vm) is a separate binary that can rollback the main binary if it crashes or is compromised.
