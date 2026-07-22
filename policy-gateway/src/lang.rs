//! 国际化模块 (i18n) — 简体中文 / English

use std::sync::OnceLock;
use std::collections::HashMap;
pub use StrKey as S;

static LANG: OnceLock<Lang> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    Zh,
    En,
}

impl Lang {
    pub fn current() -> &'static Lang {
        LANG.get_or_init(|| {
            if let Ok(v) = std::env::var("PG_LANGUAGE") {
                if v == "en" { return Lang::En; }
                if v == "zh" { return Lang::Zh; }
            }
            if let Ok(locale) = std::env::var("LANG") {
                if locale.starts_with("zh") { return Lang::Zh; }
            }
            Lang::Zh
        })
    }

    pub fn set(lang: Lang) {
        let _ = LANG.set(lang);
    }
}

pub fn t(key: StrKey) -> &'static str {
    let lang = *Lang::current();
    STRINGS[&(key, lang)]
}

static STRINGS: once_cell::sync::Lazy<HashMap<(StrKey, Lang), &'static str>> = once_cell::sync::Lazy::new(|| {
    let mut m = HashMap::new();
    fn i(m: &mut HashMap<(StrKey, Lang), &'static str>, k: StrKey, zh: &'static str, en: &'static str) {
        m.insert((k, Lang::Zh), zh);
        m.insert((k, Lang::En), en);
    }

    // === 通用 ===
    i(&mut m, StrKey::Ok, "成功", "OK");
    i(&mut m, StrKey::Error, "错误", "Error");
    i(&mut m, StrKey::Warning, "警告", "Warning");
    i(&mut m, StrKey::ConfigSaved, "✅ 配置已保存", "✅ Config saved");
    i(&mut m, StrKey::ConfigReset, "✅ 配置已重置", "✅ Config reset");
    i(&mut m, StrKey::PressEnter, "(直接回车保留当前值)", "(press Enter to keep)");

    // === 启动 ===
    i(&mut m, StrKey::GatewayStarted, "🔐 policy-gateway — 模块化核心", "🔐 policy-gateway — Modular core");
    i(&mut m, StrKey::CoreDesc, "核心功能: 证书认证 + 上网控制", "Core: Certificate auth + Internet control");
    i(&mut m, StrKey::CaGenerated, "🔑 CA 密钥 已生成", "🔑 CA key generated");
    i(&mut m, StrKey::FirstStart, "🆕 首次启动 — 权限表为空", "🆕 First start — empty permission table");
    i(&mut m, StrKey::Listening, "🌐 监听 0.0.0.0:8443", "🌐 Listening on 0.0.0.0:8443");

    // === 配置 ===
    i(&mut m, StrKey::CfgServer, "服务器地址", "Server URL");
    i(&mut m, StrKey::CfgToken, "管理 Token", "Manager Token");
    i(&mut m, StrKey::CfgPort, "监听端口", "Listen port");
    i(&mut m, StrKey::CfgHtml, "启用 HTML (y/n)", "Enable HTML (y/n)");
    i(&mut m, StrKey::CfgDb, "数据库路径", "Database path");
    i(&mut m, StrKey::CfgLang, "语言 (zh/en)", "Language (zh/en)");
    i(&mut m, StrKey::TlsHeader, "--- TLS 连接配置 ---", "--- TLS config ---");
    i(&mut m, StrKey::TlsHttp, "允许 HTTP (y/n)", "Allow HTTP (y/n)");
    i(&mut m, StrKey::Tls12, "允许 TLS 1.2 (y/n)", "Allow TLS 1.2 (y/n)");
    i(&mut m, StrKey::Tls13, "允许 TLS 1.3 (y/n)", "Allow TLS 1.3 (y/n)");
    i(&mut m, StrKey::TlsQuic, "允许 QUIC (y/n)", "Allow QUIC (y/n)");
    i(&mut m, StrKey::TlsMtls, "启用 mTLS 客户端证书验证 (y/n)", "Enable mTLS client cert (y/n)");

    // === API ===
    i(&mut m, StrKey::InvalidCsr, "CSR 格式无效", "Invalid CSR");
    i(&mut m, StrKey::NeedCsrPubkey, "请提供 csr、pubkey 或 cert", "Provide csr, pubkey or cert");
    i(&mut m, StrKey::PubkeyLen, "公钥长度应为 32(Ed25519) 或 33(P-256) 字节", "Pubkey must be 32(Ed25519) or 33(P-256) bytes");
    i(&mut m, StrKey::PubkeyNotHex, "公钥不是有效的十六进制字符串", "Pubkey is not valid hex");
    i(&mut m, StrKey::ReqNotFound, "申请 ID 不存在或已处理", "Request ID not found");
    i(&mut m, StrKey::InvalidAction, "未知操作，可用: approve / reject", "Unknown action: approve / reject");
    i(&mut m, StrKey::Unauthorized, "未授权", "Unauthorized");
    i(&mut m, StrKey::WrongRoleBit, "错误的权限位", "Invalid permission bit");

    // === HTML ===
    i(&mut m, StrKey::CertTitle, "📜 证书申请", "📜 Certificate Request");
    i(&mut m, StrKey::DeviceName, "设备名", "Device name");
    i(&mut m, StrKey::PermTemplate, "权限模板", "Permission template");
    i(&mut m, StrKey::ConnectorCert, "🌐 连接证书 (仅上网)", "🌐 Connector cert");
    i(&mut m, StrKey::DeviceCert, "⚙️ 设备证书 (上网 + 计算)", "⚙️ Device cert");
    i(&mut m, StrKey::AdminPanel, "🔑 管理面板", "🔑 Admin Panel");
    i(&mut m, StrKey::LoginTitle, "登录", "Login");
    i(&mut m, StrKey::Submit, "提交", "Submit");
    i(&mut m, StrKey::Reject, "拒绝", "Reject");
    i(&mut m, StrKey::Approve, "批准", "Approve");
    i(&mut m, StrKey::PendingCount, "待审批:", "Pending:");
    i(&mut m, StrKey::NoPending, "暂无待审批申请", "No pending requests");

    // === CLI ===
    i(&mut m, StrKey::ConfigText, "配置", "Config");
    i(&mut m, StrKey::ConfigEdit, "编辑配置", "Edit config");
    i(&mut m, StrKey::ConfigShow, "查看配置", "Show config");
    i(&mut m, StrKey::ConfigReset, "重置配置", "Reset config");
    i(&mut m, StrKey::ConfigTls, "TLS 配置", "TLS config");
    i(&mut m, StrKey::CliUsage, "用法:", "Usage:");
    i(&mut m, StrKey::CliServe, "启动网页服务", "Start web server");
    i(&mut m, StrKey::CliVersion, "显示版本", "Show version");
    i(&mut m, StrKey::CliHelp, "显示此帮助", "Show this help");
    i(&mut m, StrKey::CliPerm, "权限表操作", "Permission table");
    i(&mut m, StrKey::CliVm, "VM 管理", "VM management");
    i(&mut m, StrKey::CliInit, "首次设置", "First setup");
    i(&mut m, StrKey::CliModule, "模块信息", "Module info");
    i(&mut m, StrKey::CliPermGc, "清理过期条目", "GC expired entries");
    i(&mut m, StrKey::CliPermList, "列出所有条目", "List all entries");
    i(&mut m, StrKey::CliPermStats, "权限表统计", "Table statistics");
    i(&mut m, StrKey::CliVmInit, "初始化备份目录", "Init backup dir");
    i(&mut m, StrKey::CliVmInstall, "安装/升级主程序", "Install/upgrade");
    i(&mut m, StrKey::CliVmSnapshot, "创建快照", "Create snapshot");
    i(&mut m, StrKey::CliVmRollback, "回滚", "Rollback");
    i(&mut m, StrKey::CliVmList, "列举快照", "List snapshots");
    i(&mut m, StrKey::CliVmStatus, "查看状态", "View status");
    i(&mut m, StrKey::CliVmVerify, "校验完整性", "Verify integrity");
    i(&mut m, StrKey::CliTotal, "总条目:", "Total:");
    i(&mut m, StrKey::CliActive, "活跃:", "Active:");
    i(&mut m, StrKey::CliRevoked, "已吊销:", "Revoked:");
    i(&mut m, StrKey::CliPending, "待审批:", "Pending:");

    m
});

// ===== 键枚举 =====
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StrKey {
    // 通用
    Ok, Error, Warning, ConfigSaved, ConfigReset, PressEnter,
    // 启动
    GatewayStarted, CoreDesc, CaGenerated, FirstStart, Listening,
    // 配置
    CfgServer, CfgToken, CfgPort, CfgHtml, CfgDb, CfgLang,
    TlsHeader, TlsHttp, Tls12, Tls13, TlsQuic, TlsMtls,
    // API
    InvalidCsr, NeedCsrPubkey, PubkeyLen, PubkeyNotHex,
    ReqNotFound, InvalidAction, Unauthorized, WrongRoleBit,
    // HTML
    CertTitle, DeviceName, PermTemplate, ConnectorCert, DeviceCert,
    AdminPanel, LoginTitle, Submit, Reject, Approve,
    PendingCount, NoPending,
    // CLI
    ConfigText, ConfigEdit, ConfigShow, ConfigTls,
    CliUsage, CliServe, CliVersion, CliHelp, CliPerm, CliVm, CliInit, CliModule,
    CliPermGc, CliPermList, CliPermStats,
    CliVmInit, CliVmInstall, CliVmSnapshot, CliVmRollback, CliVmList, CliVmStatus, CliVmVerify,
    CliTotal, CliActive, CliRevoked, CliPending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keys_complete() {
        // All keys should resolve in both languages
        Lang::set(Lang::Zh);
        for key in STRINGS.keys().filter(|(_, l)| *l == Lang::Zh).map(|(k, _)| *k).collect::<std::collections::HashSet<_>>() {
            assert!(STRINGS.contains_key(&(key, Lang::En)), "Missing en for {:?}", key);
        }
    }

    #[test]
    fn test_known() {
        // Don't use set() — relies on OnceLock initialization order
        // Just verify the STRINGS map is populated
        assert!(STRINGS.contains_key(&(StrKey::Ok, Lang::Zh)));
        assert!(STRINGS.contains_key(&(StrKey::Ok, Lang::En)));
        assert_eq!(STRINGS[&(StrKey::Ok, Lang::Zh)], "成功");
        assert_eq!(STRINGS[&(StrKey::Ok, Lang::En)], "OK");
    }
}
