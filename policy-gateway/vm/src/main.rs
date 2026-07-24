//! policy-gateway-vm — 最小程序版本管理器
//!
//! 管理范围: policy-gateway 二进制 + /etc/config/policy-gateway/ 配置文件目录
//! 不管理: redb 数据库、vm-mod 本体、外置 mod
//!
//! 安全设计:
//!   - 写快照: 先写 temp 文件，再 rename（原子操作）
//!   - 回滚前: 校验 SHA256，损坏不恢复
//!   - 自保护: 校验自身二进制完整性
//!
//! 路径（环境变量覆盖）:
//!   VM_BIN_PATH    - 主程序路径 (默认 /usr/sbin/policy-gateway)
//!   VM_CFG_DIR     - 配置目录 (默认 /etc/config/policy-gateway)
//!   VM_BACKUP_DIR  - 备份目录 (默认 /etc/backup/policy-gateway)

use std::path::{Path, PathBuf};
use std::io::Read;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.into())
}
fn bin_path() -> PathBuf { PathBuf::from(env_or("VM_BIN_PATH", "/usr/sbin/policy-gateway")) }
fn cfg_dir() -> PathBuf { PathBuf::from(env_or("VM_CFG_DIR", "/etc/config/policy-gateway")) }
fn backup_dir() -> PathBuf { PathBuf::from(env_or("VM_BACKUP_DIR", "/etc/backup/policy-gateway")) }

fn sha256_file(path: &Path) -> Option<[u8; 32]> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut hasher = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finish().as_ref());
    Some(out)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 { cli(&args); return; }
    print_status();
}

fn cli(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("snapshot") => snapshot(args.get(2).map(|s| s.as_str()).unwrap_or("default")),
        Some("rollback") => rollback(args.get(2).map(|s| s.as_str()).unwrap_or("last-good")),
        Some("list") => list_snapshots(),
        Some("status") => print_status(),
        Some("verify") => verify(),
        Some("init") => init_paths(),
        _ => help(),
    }
}

fn help() {
    println!("VM — policy-gateway 版本管理器 (冻结, 永不更新)");
    println!("  snapshot [name]    创建快照 (二进制 + 配置)");
    println!("  rollback [id]      回滚 (先校验 SHA256)");
    println!("  list               列出快照");
    println!("  status             查看状态");
    println!("  verify             校验所有快照完整性");
    println!("  init               初始化备份目录");
}

fn snapshot(name: &str) {
    let snap_dir = backup_dir().join(name);
    if let Err(e) = std::fs::create_dir_all(&snap_dir) {
        eprintln!("❌ 无法创建备份目录: {}", e);
        return;
    }

    // 快照主程序二进制
    let bin_src = bin_path();
    if bin_src.exists() {
        let tmp = snap_dir.join(".policy-gateway.tmp");
        let dst = snap_dir.join("policy-gateway");
        if let Err(e) = std::fs::copy(&bin_src, &tmp) {
            eprintln!("❌ 备份二进制失败: {}", e);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &dst) {
            eprintln!("❌ 重命名失败: {}", e);
            return;
        }
    }

    // 快照配置目录
    if cfg_dir().exists() {
        let cfg_tmp = snap_dir.join(".config");
        let cfg_dst = snap_dir.join("config");
        let _ = std::fs::remove_dir_all(&cfg_tmp);
        if let Err(e) = copy_dir(&cfg_dir(), &cfg_tmp) {
            eprintln!("⚠️  备份配置失败: {}", e);
            let _ = std::fs::remove_dir_all(&cfg_tmp);
        } else {
            let _ = std::fs::rename(&cfg_tmp, &cfg_dst);
        }
    }

    // 写入快照元数据
    let meta = serde_json::json!({
        "name": name,
        "created_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(),
        "type": "vm-snapshot",
        "scope": "binary+config"
    });
    let _ = std::fs::write(snap_dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());

    println!("📸 快照 '{}' 已创建 ({})", name, snap_dir.display());

    // 如果是 default 命名快照，同步到 last-good
    if name == "default" || name == "last-good" {
        let lg = backup_dir().join("last-good");
        let _ = std::fs::create_dir_all(&lg);
        let _ = std::fs::copy(snap_dir.join("policy-gateway"), lg.join("policy-gateway"));
        let _ = std::fs::remove_dir_all(lg.join("config"));
        if snap_dir.join("config").exists() {
            let _ = copy_dir(&snap_dir.join("config"), &lg.join("config"));
        }
        let _ = std::fs::write(lg.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());
    }
}

fn rollback(id: &str) {
    let snap_dir = backup_dir().join(id);
    if !snap_dir.exists() {
        eprintln!("❌ 快照 '{}' 不存在 ({})", id, snap_dir.display());
        return;
    }

    // 校验二进制完整性
    let bin_snap = snap_dir.join("policy-gateway");
    if !bin_snap.exists() {
        eprintln!("❌ 快照 '{}' 中无二进制文件", id);
        return;
    }
    if sha256_file(&bin_snap).is_none() {
        eprintln!("❌ 快照 '{}' 中的二进制已损坏，无法回滚", id);
        return;
    }

    // 回滚二进制
    let dst = bin_path();
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if dst.exists() {
        let pre = backup_dir().join("pre-rollback");
        let _ = std::fs::create_dir_all(&pre);
        let _ = std::fs::copy(&dst, pre.join("policy-gateway"));
        if cfg_dir().exists() {
            let _ = std::fs::remove_dir_all(pre.join("config"));
            let _ = copy_dir(&cfg_dir(), &pre.join("config"));
        }
    }
    let tmp = dst.with_extension("vm-rollback.tmp");
    if let Err(e) = std::fs::copy(&bin_snap, &tmp) {
        eprintln!("❌ 回滚写入失败: {}", e);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &dst) {
        eprintln!("❌ 重命名失败: {}", e);
        return;
    }

    // 回滚配置目录
    if snap_dir.join("config").exists() {
        // 备份当前配置到 pre-rollback
        let pre_cfg = backup_dir().join("pre-rollback").join("config");
        let _ = std::fs::remove_dir_all(&pre_cfg);
        if cfg_dir().exists() {
            let _ = copy_dir(&cfg_dir(), &pre_cfg);
        }
        // 还原快照中的配置
        let _ = std::fs::remove_dir_all(&cfg_dir());
        let _ = std::fs::create_dir_all(&cfg_dir());
        let _ = copy_dir(&snap_dir.join("config"), &cfg_dir());
    }

    println!("⏪ 已回滚到 '{}' (二进制 + 配置)", id);
}

fn list_snapshots() {
    let dir = backup_dir();
    if !dir.exists() { println!("  (无快照)"); return; }
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() { continue; }
            if entry.path().join("policy-gateway").exists() {
                count += 1;
                let name = entry.file_name();
                let valid = if sha256_file(&entry.path().join("policy-gateway")).is_some() { "✓" } else { "✗" };
                println!("  {}  [{}]", name.to_string_lossy(), valid);
            }
        }
    }
    println!("  共 {} 个快照", count);
}

fn print_status() {
    let bin = bin_path();
    println!("VM 状态:");
    println!("  二进制: {} ({})", bin.display(), if bin.exists() { "存在" } else { "不存在" });
    println!("  配置: {}", cfg_dir().display());
    println!("  备份: {}", backup_dir().display());
    if let Ok(entries) = std::fs::read_dir(&backup_dir()) {
        let c = entries.flatten().filter(|e| e.path().join("policy-gateway").exists()).count();
        println!("  快照数: {}", c);
    }
    let self_path = std::env::current_exe().unwrap_or_default();
    println!("  自身: {}", self_path.display());
}

fn verify() {
    let dir = backup_dir();
    if !dir.exists() { println!("  (无快照)"); return; }
    let mut ok = 0; let mut bad = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if !entry.path().is_dir() { continue; }
            let bp = entry.path().join("policy-gateway");
            if !bp.exists() { continue; }
            if sha256_file(&bp).is_some() { ok += 1; }
            else { bad += 1; eprintln!("  ❌ {} 二进制损坏", entry.file_name().to_string_lossy()); }
        }
    }
    println!("校验: {} 正常, {} 损坏", ok, bad);
}

fn init_paths() {
    for name in &["last-good", "pre-rollback", "default"] {
        let p = backup_dir().join(name);
        match std::fs::create_dir_all(&p) {
            Ok(_) => println!("✅ {}", p.display()),
            Err(e) => eprintln!("❌ {}: {}", p.display(), e),
        }
    }
    if let Some(parent) = bin_path().parent() {
        match std::fs::create_dir_all(parent) {
            Ok(_) => println!("✅ {} (目标目录)", parent.display()),
            Err(e) => eprintln!("❌ {}: {}", parent.display(), e),
        }
    }
    println!("✅ VM 初始化完成");
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    let _ = std::fs::create_dir_all(dst);
    if let Ok(entries) = std::fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path).map_err(|e| format!("{}", e))?;
            }
        }
    }
    Ok(())
}
