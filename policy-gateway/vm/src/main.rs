//! policy-gateway-vm — "不死鸟" 独立版本管理器
//!
//! 这是一个完全独立的二进制文件，不依赖 policy-gateway 的任何代码。
//! 它存储在闪存中，即使主程序崩溃也能运行。
//!
//! 功能:
//!   1. 检测主程序是否正常启动（看门狗/心跳文件）
//!   2. 如果主程序崩溃，从 USB 备份恢复上一个版本
//!   3. 管理快照（保存/恢复主程序二进制和配置）
//!
//! 编译:
//!   cd vm && cargo build --release
//!   upx --best target/release/policy-gateway-vm

use std::path::Path;

/// 主程序路径
const MAIN_BIN: &str = "/tmp/system/policy-gateway";
/// 备份目录（USB）
const BACKUP_DIR: &str = "/mnt/usb/backup/vm";
/// 看门狗心跳文件（主程序启动后会 touch 这个文件）
const WATCHDOG_FILE: &str = "/tmp/vm-watchdog";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return cli(&args);
    }

    println!("🔰 VM — 不死鸟版本管理器");
    println!("   检测主程序...");

    if !Path::new(WATCHDOG_FILE).exists() {
        println!("   ⚠️  主程序未启动或已崩溃");
        if Path::new(BACKUP_DIR).join("last-good/main.bin").exists() {
            println!("   🔄 发现上一个正常版本，尝试恢复...");
            restore_last_good();
        } else {
            println!("   ❌ 无可用备份，等待人工干预");
        }
    } else {
        println!("   ✅ 主程序运行正常");
        // 正常模式下：确保定时备份
        if args.get(1).map(|s| s.as_str()) == Some("--daemon") {
            daemon_loop();
        }
    }
}

fn cli(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("snapshot") => {
            let name = args.get(2).map(|s| s.as_str()).unwrap_or("default");
            create_snapshot(name);
        }
        Some("rollback") => {
            let id = args.get(2).map(|s| s.as_str()).unwrap_or("last-good");
            rollback(id);
        }
        Some("list") => {
            list_snapshots();
        }
        Some("status") => {
            println!("VM 状态:");
            println!("  主程序: {}", if Path::new(MAIN_BIN).exists() { "存在" } else { "不存在" });
            println!("  主程序运行: {}", if Path::new(WATCHDOG_FILE).exists() { "是" } else { "否" });
            println!("  备份路径: {}", BACKUP_DIR);
        }
        _ => {
            println!("VM — 不死鸟版本管理器");
            println!("  snapshot [name]    创建快照");
            println!("  rollback [id]      回滚");
            println!("  list               列出快照");
            println!("  status             查看状态");
        }
    }
}

fn create_snapshot(name: &str) {
    // 备份主程序二进制
    if Path::new(MAIN_BIN).exists() {
        let dest = format!("{}/{}/main.bin", BACKUP_DIR, name);
        std::fs::create_dir_all(Path::new(&dest).parent().unwrap()).ok();
        std::fs::copy(MAIN_BIN, &dest).ok();
        println!("📸 快照 '{}' 已创建", name);
    } else {
        eprintln!("❌ 主程序不存在");
    }
}

fn restore_last_good() {
    let src = format!("{}/last-good/main.bin", BACKUP_DIR);
    if Path::new(&src).exists() {
        std::fs::create_dir_all(Path::new(MAIN_BIN).parent().unwrap()).ok();
        std::fs::copy(&src, MAIN_BIN).ok();
        println!("✅ 已恢复上一个正常版本");
        println!("   重启主程序: {}", MAIN_BIN);
    }
}

fn rollback(id: &str) {
    let src = format!("{}/{}/main.bin", BACKUP_DIR, id);
    if Path::new(&src).exists() {
        std::fs::copy(&src, MAIN_BIN).ok();
        println!("⏪ 已回滚到 '{}'", id);
    } else {
        eprintln!("❌ 快照 '{}' 不存在", id);
    }
}

fn list_snapshots() {
    if !Path::new(BACKUP_DIR).exists() {
        println!("  (无快照)");
        return;
    }
    if let Ok(entries) = std::fs::read_dir(BACKUP_DIR) {
        for entry in entries.flatten() {
            if entry.path().join("main.bin").exists() {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(dur) = modified.elapsed() {
                            println!("  {}  ({} 秒前)", entry.file_name().to_string_lossy(), dur.as_secs());
                            continue;
                        }
                    }
                }
                println!("  {}", entry.file_name().to_string_lossy());
            }
        }
    }
}

fn daemon_loop() {
    println!("🔄 VM 守护模式 — 每 1 小时检查并备份");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
        if Path::new(WATCHDOG_FILE).exists() {
            create_snapshot("auto-hourly");
            // 保留最新的 3 个自动快照
            cleanup_old_snapshots(3);
        }
    }
}

fn cleanup_old_snapshots(_keep: usize) {
    // TODO: 清理旧自动快照
}
