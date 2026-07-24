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

use std::path::{PathBuf, Path};

fn cfg_path() -> PathBuf {
    std::env::var("PG_CONFIG_DIR")
        .map(|s| PathBuf::from(s))
        .unwrap_or_else(|_| PathBuf::from("/etc/config/policy-gateway"))
}

fn modules_dir() -> PathBuf {
    std::env::var("PG_MODULES_DIR")
        .unwrap_or_else(|_| "/mnt/usb/modules".to_string())
        .into()
}

fn backup_dir() -> PathBuf {
    std::env::var("PG_BACKUPS_DIR")
        .unwrap_or_else(|_| "/etc/backup/policy-gateway/vm-mod-snapshots".to_string())
        .into()
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 { print_help(); return; }

    match args[1].as_str() {
        "install" if args.len() > 2 => cmd_install(&args[2]),
        "remove" | "uninstall" if args.len() > 2 => cmd_remove(&args[2]),
        "list" | "ls" => cmd_list(),
        "info" if args.len() > 2 => cmd_info(&args[2]),
        "update" if args.len() > 2 => cmd_update(&args[2]),
        "snapshot" if args.len() > 2 => cmd_snapshot(&args[2]),
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
    println!("  vm-mod snapshot <name>       创建模块快照");
    println!("  vm-mod rollback <name>        回滚模块版本");
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
    let mod_dir = modules_dir().join(name);
    let _ = std::fs::create_dir_all(&mod_dir);

    // 如果 name 以路径形式给出，复制模块文件
    let src = std::path::Path::new(name);
    if src.exists() && src.is_dir() {
        let _ = copy_dir(src, &mod_dir);
    } else if src.exists() && src.is_file() {
        let _ = std::fs::copy(src, mod_dir.join("module.bin"));
    }

    // 注册到 cli-registry.toml
    register_commands(name);

    println!("📦 模块 '{name}' 已安装到 {}", mod_dir.display());
}

fn cmd_remove(name: &str) {
    let dir = modules_dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    unregister_commands(name);
    println!("🗑️  模块 '{name}' 已移除");
}

fn register_commands(name: &str) {
    let reg_dir = cfg_path().join("cli-registry");
    let _ = std::fs::create_dir_all(&reg_dir);
    let mod_dir = modules_dir().join(name);
    let entry = format!(
        "name = \"{name}\"\nexec = \"{path}/command\"\ndescription = \"{name} module\"\n",
        name = name,
        path = mod_dir.display()
    );
    let _ = std::fs::write(reg_dir.join(format!("{name}.toml")), entry);
}

fn unregister_commands(name: &str) {
    let reg_file = cfg_path().join("cli-registry").join(format!("{name}.toml"));
    let _ = std::fs::remove_file(&reg_file);
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
    let mod_dir = modules_dir().join(name);
    if !mod_dir.exists() {
        eprintln!("❌ 模块 '{name}' 未安装");
        return;
    }
    // 重新注册指令集 (模块自身的 commands.toml)
    let cmd_file = mod_dir.join("commands.toml");
    if cmd_file.exists() {
        let reg_dir = cfg_path().join("cli-registry");
        let _ = std::fs::create_dir_all(&reg_dir);
        if let Ok(content) = std::fs::read_to_string(&cmd_file) {
            let _ = std::fs::write(reg_dir.join(format!("{name}.toml")), content);
            println!("⬆️  模块 '{name}' 指令集已更新");
        }
    } else {
        println!("⬆️  模块 '{name}' 已升级 (无 commands.toml，指令集不变)");
    }
}

fn cmd_snapshot(name: &str) {
    let src = modules_dir();
    let snap_dir = backup_dir().join(name);
    if !src.exists() {
        eprintln!("⚠️  模块目录不存在，创建空快照");
    }
    let _ = std::fs::create_dir_all(&snap_dir);
    if src.exists() {
        let _ = std::fs::remove_dir_all(snap_dir.join("modules"));
        if let Err(e) = copy_dir(&src, &snap_dir.join("modules")) {
            eprintln!("❌ 快照失败: {}", e);
            return;
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let meta = serde_json::json!({"name": name, "created_at": ts, "scope": "vm-mod"});
    let _ = std::fs::write(snap_dir.join("meta.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());
    println!("📸 vm-mod snapshot '{}' saved", name);
}

fn cmd_rollback(name: &str) {
    let snap_dir = backup_dir().join(name);
    let dst = modules_dir();
    if !snap_dir.exists() || !snap_dir.join("modules").exists() {
        eprintln!("❌ 快照 '{}' 不存在", name);
        return;
    }
    let _ = std::fs::remove_dir_all(&dst);
    let _ = std::fs::create_dir_all(&dst);
    if let Err(e) = copy_dir(&snap_dir.join("modules"), &dst) {
        eprintln!("❌ 回滚失败: {}", e);
        return;
    }
    println!("⏪ vm-mod rollback '{}' done", name);
}
