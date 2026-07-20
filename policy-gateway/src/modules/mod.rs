pub mod portal;
// 模块系统 — 核心可扩展框架
//
// 核心只做认证+门户，所有业务功能（计算、存储、前端）都是模块。
// 模块可以放在闪存、USB、甚至从对象存储加载。

use axum::Router;
use std::sync::Arc;

use crate::auth::AuthTable;
use crate::anti_abuse::AntiAbuse;
use tokio::sync::RwLock;

// 共享的核心状态，传递给每个模块
#[derive(Clone)]
pub struct CoreState {
    pub auth_table: Arc<RwLock<AuthTable>>,
    pub anti_abuse: Arc<RwLock<AntiAbuse>>,
}

// 模块定义：一个模块 = 名 + 路由 + 初始化
pub trait GatewayModule: Send + Sync {
    fn name(&self) -> &'static str;
    /// 模块需要的权限位（如需要 connector 才能访问该模块）
    fn required_permission(&self) -> Option<u8> { None }
    /// 注册路由到核心路由器
    fn mount(&self, state: CoreState) -> Router;
}

// 模块清单
pub struct ModuleRegistry {
    pub modules: Vec<Box<dyn GatewayModule>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { modules: Vec::new() }
    }

    /// 注册模块
    pub fn register(&mut self, module: Box<dyn GatewayModule>) {
        log::info!("📦 加载模块: {}", module.name());
        self.modules.push(module);
    }

    /// 从文件系统扫描并加载模块（Phase 2）
    /// 扫描 /mnt/usb/modules/*/module.json
    pub fn scan_paths(_paths: &[&str]) -> Self {
        // TODO Phase 2: 扫描 USB / 对象存储加载模块
        ModuleInfo::log_available();
        Self { modules: Vec::new() }
    }
}

// 模块信息（用于发现和加载）
pub struct ModuleInfo;

impl ModuleInfo {
    pub fn log_available() {
        log::info!("📦 模块系统就绪。将模块放入 /mnt/usb/modules/ 或通过 Worker R2 下发。");
        log::info!("   模块清单格式: /mnt/usb/modules/<name>/module.toml");
        log::info!("   模块可以是: Rust 二进制、WASM、Shell 脚本、挂载目录");
    }
}

// 计算模块已从核心剥离
// 见 modules/compute/ 目录
pub fn compute_module_info() -> &'static str {
    "计算模块 — 已剥离为独立模块。需要在路由器上部署 compute-daemon 或通过 Worker 调度。"
}
