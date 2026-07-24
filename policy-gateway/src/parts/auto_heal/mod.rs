#![allow(dead_code)]
//! auto-heal — 崩溃自愈模块 (可选编译)
//!
//! 服务器异常退出后自动重建 nftables 规则，恢复服务。
//! 不编译此模块时，服务器退出后 nftables 规则不会被清理，
//! 但也不会自动重建 — 需手动重启或依赖 procd 重启。

use axum::Router;
use crate::parts::{CoreState, GatewayModule, ModuleDeclaration};

pub struct AutoHeal;

impl AutoHeal {
    pub fn new() -> Self { Self }
}

impl GatewayModule for AutoHeal {
    fn name(&self) -> &'static str { "auto-heal" }

    fn declaration(&self) -> ModuleDeclaration {
        ModuleDeclaration {
            name: "auto-heal", version: 1, claim_bits: vec![],
            min_core_version: "0.3.6",
        }
    }
    fn mount(&self, _state: CoreState) -> Router { Router::new() }
}
