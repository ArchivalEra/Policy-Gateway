#![allow(dead_code)]
//! tls-listener — TLS 传输加密模块 (可选编译)
//!
//! 为管理器提供 HTTPS 支持，保护 Manager Token 不在网络明文传输。
//! 需要 config.toml 中配置 tls_cert 和 tls_key。
//!
//! 不作为模块编译时，服务以降级 HTTP 运行，不影响其他功能。

use axum::Router;
use crate::modules::{CoreState, GatewayModule, ModuleDeclaration};

pub struct TlsListener;

impl TlsListener {
    pub fn new() -> Self { Self }
}

impl GatewayModule for TlsListener {
    fn name(&self) -> &'static str { "tls-listener" }

    fn declaration(&self) -> ModuleDeclaration {
        ModuleDeclaration {
            name: "tls-listener", version: 1, claim_bits: vec![],
            min_core_version: "0.3.6",
        }
    }
    fn mount(&self, _state: CoreState) -> Router { Router::new() }
}
