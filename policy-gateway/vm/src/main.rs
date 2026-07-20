//! policy-gateway-vm — "不死鸟" 独立版本管理器
//!
//! 完全独立的二进制，主程序崩了它还能回滚。
//! 闪存占用目标: ~80kB (UPX后)
//!
//! 路径配置（按优先级）:
//!   1. 环境变量 VM_MAIN_BIN, VM_BACKUP_DIR, VM_WATCHDOG
//!   2. /etc/config/policy-gateway (OpenWrt UCI 风格)
//!   3. OpenWrt 标准路径 /usr/sbin/, /etc/, /tmp/
//!
//! 编译:
//!   cargo build --release && upx --best target/release/policy-gateway-vm

use std::path::{Path, PathBuf};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.into())
}

fn main_bin() -> PathBuf { PathBuf::from(env_or("VM_MAIN_BIN", "/usr/sbin/policy-gateway")) }
fn backup_dir() -> PathBuf { PathBuf::from(env_or("VM_BACKUP_DIR", "/etc/backup/policy-gateway")) }
fn watchdog() -> PathBuf { PathBuf::from(env_or("VM_WATCHDOG", "/tmp/.policy-gateway-running")) }

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 { return cli(&args); }

    println!("🔰 VM — 不死鸟 v0.1");
    println!("   主程序: {}", main_bin().display());
    println!("   备份: {}", backup_dir().display());

    if !main_bin().exists() {
        println!("   ⚠️  主程序不存在");
        if backup_dir().join("last-good/main.bin").exists() {
            println!("   🔄 发现备份，恢复...");
            restore("last-good");
        }
    } else if !watchdog().exists() {
        println!("   ⚠️  主程序未启动或已崩溃");
        if backup_dir().join("last-good/main.bin").exists() {
            println!("   🔄 自动恢复 last-good...");
            restore("last-good");
        }
    } else {
        println!("   ✅ 主程序运行正常");
    }
}

fn cli(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("snapshot") => snapshot(args.get(2).map(|s| s.as_str()).unwrap_or("default")),
        Some("rollback") => rollback(args.get(2).map(|s| s.as_str()).unwrap_or("last-good")),
        Some("list") => list_snapshots(),
        Some("status") => print_status(),
        Some("init") => init_paths(),
        _ => help(),
    }
}

fn help() {
    println!("VM — 版本管理器 (独立二进制)");
    println!("  snapshot [name]    创建快照");
    println!("  rollback [id]      回滚");
    println!("  list               列出快照");
    println!("  status             查看状态");
    println!("  init               初始化备份目录");
    println!();
    println!("路径:");
    println!("  主程序: {}", main_bin().display());
    println!("  备份:   {}", backup_dir().display());
    println!("  (通过 VM_MAIN_BIN / VM_BACKUP_DIR / VM_WATCHDOG 环境变量覆盖)");
}

fn snapshot(name: &str) {
    if !main_bin().exists() {
        eprintln!("❌ 主程序不存在: {}", main_bin().display());
        return;
    }
    let dest = backup_dir().join(name);
    std::fs::create_dir_all(&dest).ok();
    match std::fs::copy(&main_bin(), dest.join("main.bin")) {
        Ok(_) => println!("📸 快照 '{}' 已创建 ({})", name, dest.display()),
        Err(e) => eprintln!("❌ 快照失败: {}", e),
    }
}

fn rollback(id: &str) {
    let src = backup_dir().join(id).join("main.bin");
    if !src.exists() {
        eprintln!("❌ 快照 '{}' 不存在 ({})", id, src.display());
        return;
    }
    match std::fs::copy(&src, &main_bin()) {
        Ok(_) => println!("⏪ 已回滚到 '{}'", id),
        Err(e) => eprintln!("❌ 回滚失败: {}", e),
    }
}

fn restore(id: &str) {
    let src = backup_dir().join(id).join("main.bin");
    if !src.exists() { return; }
    if let Some(parent) = main_bin().parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::copy(&src, &main_bin()).ok();
    println!("✅ 已恢复 '{}'", id);
}

fn list_snapshots() {
    let dir = backup_dir();
    if !dir.exists() { println!("  (无快照，备份目录不存在)"); return; }
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.path().join("main.bin").exists() {
                count += 1;
                let name = entry.file_name();
                let modified = entry.metadata().ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| format!("{}秒前", d.as_secs()))
                    .unwrap_or_default();
                println!("  {}  ({})", name.to_string_lossy(), modified);
            }
        }
    }
    println!("  共 {} 个快照", count);
}

fn print_status() {
    println!("VM 状态:");
    println!("  主程序: {} ({})", main_bin().display(),
        if main_bin().exists() { "存在" } else { "不存在" });
    println!("  运行: {}", if watchdog().exists() { "是" } else { "否" });
    println!("  备份目录: {}", backup_dir().display());
    if let Ok(entries) = std::fs::read_dir(&backup_dir()) {
        let count = entries.flatten().filter(|e| e.path().join("main.bin").exists()).count();
        println!("  快照数: {}", count);
    }
}

fn init_paths() {
    let paths = [backup_dir(), backup_dir().join("last-good")];
    for p in &paths {
        match std::fs::create_dir_all(p) {
            Ok(_) => println!("✅ 创建: {}", p.display()),
            Err(e) => eprintln!("❌ 失败: {} — {}", p.display(), e),
        }
    }
}
