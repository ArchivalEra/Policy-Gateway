//! vm-mod — 模块管理器
//!
//! 管理所有模块的指令集注册/注销/版本管理。
//! 模块可安装在本地 (USB/闪存) 或远程 (Cloudflare Worker)。
//!
//! 核心指令:
//!   vm-mod install <name>    安装模块 (本地路径或 worker:前缀)
//!   vm-mod remove <name>     移除模块
//!   vm-mod list              列出所有已安装模块及版本
//!   vm-mod info <name>       查看模块详情和可用指令
//!   vm-mod update <name>     升级模块
//!   vm-mod rollback <name>   回滚模块版本

use std::path::PathBuf;

fn modules_dir() -> PathBuf {
    PathBuf::from("/mnt/usb/modules")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 { print_help(); return; }

    match args[1].as_str() {
        "install" if args.len() > 2 => cmd_install(&args[2]),
        "remove" | "uninstall" if args.len() > 2 => cmd_remove(&args[2]),
        "list" | "ls" => cmd_list(),
        "info" if args.len() > 2 => cmd_info(&args[2]),
        "update" if args.len() > 2 => cmd_update(&args[2]),
        "rollback" if args.len() > 2 => cmd_rollback(&args[2]),
        "help" | "--help" | "-h" => print_help(),
        _ => eprintln!("vm-mod: '{}' 不是 vm-mod 指令。使用 'vm-mod help' 查看可用指令。", args[1]),
    }
}

fn print_help() {
    println!("vm-mod — 模块管理器");
    println!();
    println!("  vm-mod install <name>       安装模块");
    println!("  vm-mod remove <name>        移除模块");
    println!("  vm-mod list                 列出所有模块");
    println!("  vm-mod info <name>          模块详情");
    println!("  vm-mod update <name>        升级模块");
    println!("  vm-mod rollback <name>       回滚模块");
    println!();
    println!("安装来源:");
    println!("  本地:    vm-mod install storage-more");
    println!("  Worker:  vm-mod install worker:mirror");
    println!("  URL:     vm-mod install https://example.com/module.tar.gz");
    println!();
    println!("模块安装后自动注册其指令集到 pg 的 help 中。");
    println!("模块指令集格式: /mnt/usb/modules/<name>/commands.toml");
}

fn cmd_install(name: &str) {
    let target = if name.starts_with("worker:") {
        format!("Worker: {}", &name[7..])
    } else if name.starts_with("http") {
        format!("URL: {}", name)
    } else {
        format!("Local: {}", name)
    };
    println!("📦 安装模块: {target}");
    println!("   暂未实现 — Phase 4");
}

fn cmd_remove(name: &str) {
    let dir = modules_dir().join(name);
    if dir.exists() {
        println!("🗑️  移除模块: {name}");
        println!("   暂未实现 — Phase 4");
    } else {
        eprintln!("❌ 模块 '{name}' 未安装");
    }
}

fn cmd_list() {
    let dir = modules_dir();
    println!("📦 已安装模块:");
    if !dir.exists() {
        println!("   (无)");
        return;
    }
    for entry in std::fs::read_dir(&dir).unwrap_or_else(|_| std::fs::read_dir("/tmp").unwrap()) {
        if let Ok(e) = entry {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                println!("  - {}", e.file_name().to_string_lossy());
            }
        }
    }
}

fn cmd_info(name: &str) {
    let dir = modules_dir().join(name);
    if !dir.exists() {
        eprintln!("❌ 模块 '{name}' 未安装");
        return;
    }
    let meta = dir.join("module.json");
    if meta.exists() {
        if let Ok(content) = std::fs::read_to_string(&meta) {
            println!("📄 {name}:");
            println!("{content}");
        }
    } else {
        println!("📄 {name} (module.json 不存在)");
    }
}

fn cmd_update(name: &str) {
    println!("⬆️  升级模块: {name}");
    println!("   暂未实现 — Phase 4");
}

fn cmd_rollback(name: &str) {
    println!("⏪ 回滚模块: {name}");
    println!("   暂未实现 — Phase 4");
}
