pub mod portal;
pub mod storage_more;
pub mod dns_local;
pub mod worker_sync;
// 模块系统 — 核心可扩展框架
//
// 核心只做认证+门户，所有业务功能（计算、存储、前端）都是模块。
// 模块可以放在闪存、USB、甚至从对象存储加载。
//
// 核心冻结线 (Phase 3.4):
//   核心权限表: HashMap<[u8;32], PermissionEntry> — 永不变
//   持久化后端: redb 单文件 — 永不变
//   降级模式: 纯内存 — 永不变
//   新增后端 → storage-more 模块

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
    pub ca_key_pem: String,
    pub ca_cert_pem: String,
    pub event_log: Arc<RwLock<crate::event_log::EventLog>>,
    pub serve_html: bool,
}

/// 模块声明：一个模块 = 名 + 版本 + 需要的权限位
pub struct ModuleDeclaration {
    pub name: &'static str,
    pub version: u32,
    /// 模块声明的权限位（如 [3, 4] 表示占用 bit 3 和 4）
    pub claim_bits: Vec<u8>,
    /// 最低核心版本要求（semver）
    pub min_core_version: &'static str,
}

// 模块定义
pub trait GatewayModule: Send + Sync {
    fn name(&self) -> &'static str;
    /// 模块声明（含版本+bit claim）
    fn declaration(&self) -> ModuleDeclaration;
    /// 模块需要的权限位（如需要 connector 才能访问该模块）
    fn required_permission(&self) -> Option<u8> { None }
    /// 注册路由到核心路由器
    fn mount(&self, state: CoreState) -> Router;
}

/// 模块注册表，带 bit claim 校验
pub struct ModuleRegistry {
    pub modules: Vec<Box<dyn GatewayModule>>,
    /// 已被声明的 bit 列表（避免冲突）
    claimed_bits: Vec<u8>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { modules: Vec::new(), claimed_bits: vec![0, 1, 2] } // core bits 0-2 pre-claimed
    }

    /// 注册模块（含 bit claim 校验）
    pub fn register(&mut self, module: Box<dyn GatewayModule>) -> Result<(), String> {
        let decl = module.declaration();
        
        // 检查版本兼容性
        let core_ver = env!("CARGO_PKG_VERSION");
        if decl.min_core_version > core_ver {
            return Err(format!(
                "模块 '{}' v{} 需要核心 >= {}, 当前核心 {} — 跳过加载",
                decl.name, decl.version, decl.min_core_version, core_ver
            ));
        }

        // 检查 bit 冲突
        for bit in &decl.claim_bits {
            if self.claimed_bits.contains(bit) {
                return Err(format!(
                    "模块 '{}' 声明的 bit {} 已被占用 — 跳过加载",
                    decl.name, bit
                ));
            }
        }

        // 注册 bit
        for bit in &decl.claim_bits {
            self.claimed_bits.push(*bit);
        }

        log::info!("📦 加载模块: {} v{} (bits: {:?})", decl.name, decl.version, decl.claim_bits);
        self.modules.push(module);
        Ok(())
    }

    /// 获取所有已声明的 bit（用于 GC：未声明的 bit 视为 locked）
    pub fn active_bits(&self) -> &[u8] {
        &self.claimed_bits
    }

    /// 返回不能写入的 locked bit 列表（0-63 中除了 active_bits 之外的）
    pub fn locked_bits(&self) -> Vec<u8> {
        let active = self.active_bits();
        (0..64).filter(|b| !active.contains(b)).collect()
    }

    /// 扫描文件系统模块路径
    pub fn scan_paths(_paths: &[&str]) -> Self {
        ModuleInfo::log_available();
        Self { modules: Vec::new(), claimed_bits: vec![0, 1, 2] }
    }
}

pub struct ModuleInfo;

impl ModuleInfo {
    pub fn log_available() {
        log::info!("📦 模块系统就绪。将模块放入 /mnt/usb/modules/ 或通过 Worker R2 下发。");
        log::info!("   模块清单格式: /mnt/usb/modules/<name>/module.toml");
    }
}

