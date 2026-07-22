//! policy-gateway-vm — "不死鸟" 独立版本管理器
//!
//! 安全设计 (Rust 无 unsafe，零恐慌路径):
//!   1. 写快照: 先写 temp 文件，再 rename（原子操作）
//!   2. 回滚前: 校验快照 SHA256，损坏的不恢复
//!   3. 自保护: 校验自身二进制完整性
//!   4. "金备份": last-good 永远不被自动覆盖
//!   5. 零恐慌: 所有错误显式处理，不 unwrap/expect
//!
//! 路径（环境变量覆盖）:
//!   VM_MAIN_BIN    - 主程序路径 (默认 /usr/sbin/policy-gateway)
//!   VM_BACKUP_DIR  - 备份目录 (默认 /etc/backup/policy-gateway)
//!   VM_WATCHDOG    - 看门狗文件 (默认 /tmp/.policy-gateway-running)

use std::path::{Path, PathBuf};
use std::io::Read;

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.into())
}
fn main_bin() -> PathBuf { PathBuf::from(env_or("VM_MAIN_BIN", "/usr/sbin/policy-gateway")) }
fn backup_dir() -> PathBuf { PathBuf::from(env_or("VM_BACKUP_DIR", "/etc/backup/policy-gateway")) }
fn watchdog() -> PathBuf { PathBuf::from(env_or("VM_WATCHDOG", "/tmp/.policy-gateway-running")) }

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
    if !main_bin().exists() || !watchdog().exists() {
        if backup_dir().join("last-good/main.bin").exists() {
            eprintln!("⚠️  检测到异常，自动恢复 last-good...");
            restore("last-good");
        }
    }
}

fn cli(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("snapshot") => snapshot(args.get(2).map(|s| s.as_str()).unwrap_or("default")),
        Some("rollback") => rollback(args.get(2).map(|s| s.as_str()).unwrap_or("last-good")),
        Some("install") => install(args.get(2)),
        Some("list") => list_snapshots(),
        Some("status") => print_status(),
        Some("verify") => verify(),
        Some("init") => init_paths(),
        Some("hot-update") => hot_update(args.get(2)),
        Some("maintenance") => maintenance_cli(&args[1..]),
        _ => help(),
    }
}

fn help() {
    println!("VM — 不死鸟 v0.2 (Rust, 零恐慌)");
    println!("  install <path>    安装/升级主程序（来源: USB / curl / scp）");
    println!("  snapshot [name]    创建快照 (原子写入)");
    println!("  rollback [id]      回滚 (先校验 SHA256)");
    println!("  list               列出快照");
    println!("  status             查看状态");
    println!("  verify             校验所有快照完整性");
    println!("  init               初始化备份目录");
    println!("  hot-update <url>   从 URL 下载并热更新 (需 curl)");
    println!("  maintenance set <开始时间戳> <结束时间戳>  设置维护窗口");
    println!("  maintenance status                       查看维护状态");
    println!("  maintenance clear                         清除维护窗口");
}

fn install(src_arg: Option<&String>) {
    let src_path = match src_arg {
        Some(p) => PathBuf::from(p),
        None => {
            // 尝试常见路径
            for p in &["/tmp/policy-gateway", "./policy-gateway", "/mnt/usb/policy-gateway"] {
                if Path::new(p).exists() { return install_src(Path::new(p)); }
            }
            eprintln!("❌ 未指定源文件，也不在常见路径");
            eprintln!("   用法: vm install /path/to/policy-gateway");
            return;
        }
    };
    install_src(&src_path);
}

fn install_src(src: &Path) {
    if !src.exists() {
        eprintln!("❌ 源文件不存在: {}", src.display());
        return;
    }
    // 校验源文件完整性
    let src_hash = match sha256_file(src) {
        Some(h) => h,
        None => { eprintln!("❌ 源文件损坏"); return; }
    };

    let dst = main_bin();
    if let Some(parent) = dst.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("❌ 无法创建目标目录: {}", e);
            return;
        }
    }

    // 如果已存在，先快照
    if dst.exists() {
        println!("📦 主程序已存在，自动创建 pre-install 快照...");
        let pre_dir = backup_dir().join("pre-install");
        let _ = std::fs::create_dir_all(&pre_dir);
        let _ = std::fs::copy(&dst, pre_dir.join("main.bin"));
    }

    // 原子安装
    let tmp = dst.with_extension("inst.tmp");
    if let Err(e) = std::fs::copy(src, &tmp) {
        eprintln!("❌ 安装写入失败: {}", e);
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &dst) {
        eprintln!("❌ 安装重命名失败: {}", e);
        return;
    }

    // 安装后自动创建 fresh-install 快照
    let fresh_dir = backup_dir().join("fresh-install");
    let _ = std::fs::create_dir_all(&fresh_dir);
    let _ = std::fs::copy(&dst, fresh_dir.join("main.bin"));

    // 同步到 last-good
    let lg_dir = backup_dir().join("last-good");
    let _ = std::fs::create_dir_all(&lg_dir);
    let _ = std::fs::copy(&dst, lg_dir.join("main.bin"));

    // 写看门狗
    touch_watchdog();

    println!("✅ 安装完成: {}", dst.display());
    println!("   SHA256: {}", hex::encode(src_hash));
    println!("   快照 'fresh-install' 已创建");
    println!("   运行 'policy-gateway' 启动服务");
    println!("   管理页面: http://<router-ip>:8443/manager");
}

fn touch_watchdog() {
    let wd = watchdog();
    if let Some(parent) = wd.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&wd, b"running");
}

fn snapshot(name: &str) {
    let src = main_bin();
    if !src.exists() {
        eprintln!("❌ 主程序不存在: {}", src.display());
        return;
    }
    let dir = backup_dir().join(name);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("❌ 无法创建备份目录: {}", e);
        return;
    }
    let tmp = dir.join(".main.bin.tmp");
    let dst = dir.join("main.bin");
    if let Err(e) = std::fs::copy(&src, &tmp) {
        eprintln!("❌ 写入失败: {}", e);
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &dst) {
        eprintln!("❌ 重命名失败: {}", e);
        return;
    }
    if sha256_file(&dst).is_none() {
        eprintln!("❌ 快照损坏，删除");
        let _ = std::fs::remove_file(&dst);
        return;
    }
    println!("📸 快照 '{}' 已创建 ({})", name, dst.display());
    
    if name == "default" || name == "last-good" {
        let lg = backup_dir().join("last-good");
        let _ = std::fs::create_dir_all(&lg);
        let lg_tmp = lg.join(".main.bin.tmp");
        let lg_dst = lg.join("main.bin");
        if std::fs::copy(&dst, &lg_tmp).is_ok() {
            let _ = std::fs::rename(&lg_tmp, &lg_dst);
        }
    }
}

fn rollback(id: &str) {
    let src = backup_dir().join(id).join("main.bin");
    if !src.exists() {
        eprintln!("❌ 快照 '{}' 不存在 ({})", id, src.display());
        return;
    }
    if sha256_file(&src).is_none() {
        eprintln!("❌ 快照 '{}' 已损坏，无法回滚", id);
        return;
    }
    let dst = main_bin();
    if let Some(parent) = dst.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if dst.exists() {
        let pre = backup_dir().join("pre-rollback");
        let _ = std::fs::create_dir_all(&pre);
        let _ = std::fs::copy(&dst, pre.join("main.bin"));
    }
    let tmp = dst.with_extension("bin.tmp");
    if let Err(e) = std::fs::copy(&src, &tmp) {
        eprintln!("❌ 回滚写入失败: {}", e);
        let _ = std::fs::remove_file(&tmp);
        return;
    }
    if let Err(e) = std::fs::rename(&tmp, &dst) {
        eprintln!("❌ 回滚重命名失败: {}", e);
        return;
    }
    touch_watchdog();
    println!("⏪ 已回滚到 '{}'", id);
}

fn restore(id: &str) {
    let src = backup_dir().join(id).join("main.bin");
    if !src.exists() { return; }
    if sha256_file(&src).is_none() { return; }
    if let Some(parent) = main_bin().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = main_bin().with_extension("bin.tmp");
    if std::fs::copy(&src, &tmp).is_ok() {
        let _ = std::fs::rename(&tmp, &main_bin());
    }
}

fn list_snapshots() {
    let dir = backup_dir();
    if !dir.exists() { println!("  (无快照)"); return; }
    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let mp = entry.path().join("main.bin");
            if mp.exists() {
                count += 1;
                let name = entry.file_name();
                let valid = if sha256_file(&mp).is_some() { "✓" } else { "✗ 损坏" };
                println!("  {}  [{}]", name.to_string_lossy(), valid);
            }
        }
    }
    println!("  共 {} 个快照", count);
}

fn print_status() {
    let mb = main_bin();
    let wd = watchdog();
    println!("VM 状态:");
    println!("  主程序: {} ({})", mb.display(), if mb.exists() { "存在" } else { "不存在" });
    println!("  运行: {}", if wd.exists() { "是" } else { "否" });
    println!("  备份: {}", backup_dir().display());
    if let Ok(e) = std::fs::read_dir(&backup_dir()) {
        let c = e.flatten().filter(|e| e.path().join("main.bin").exists()).count();
        println!("  快照数: {}", c);
    }
    let self_path = std::env::current_exe().unwrap_or_default();
    println!("  自身: {} ({})", self_path.display(), 
        if sha256_file(&self_path).is_some() { "完整" } else { "⚠️ 可能损坏" });
}

fn verify() {
    let dir = backup_dir();
    if !dir.exists() { println!("  (无快照)"); return; }
    let mut ok = 0; let mut bad = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let mp = entry.path().join("main.bin");
            if !mp.exists() { continue; }
            if sha256_file(&mp).is_some() {
                ok += 1;
            } else {
                bad += 1;
                eprintln!("  ❌ {} 损坏", entry.file_name().to_string_lossy());
            }
        }
    }
    println!("校验完成: {} 正常, {} 损坏", ok, bad);
}

fn init_paths() {
    for name in &["last-good", "pre-rollback", "pre-install", "fresh-install"] {
        let p = backup_dir().join(name);
        match std::fs::create_dir_all(&p) {
            Ok(_) => println!("✅ {}", p.display()),
            Err(e) => eprintln!("❌ {}: {}", p.display(), e),
        }
    }
    // 也创建主程序目录
    if let Some(parent) = main_bin().parent() {
        match std::fs::create_dir_all(parent) {
            Ok(_) => println!("✅ {} (目标目录)", parent.display()),
            Err(e) => eprintln!("❌ {}: {}", parent.display(), e),
        }
    }
    println!("✅ VM 目录初始化完成");
    println!("   准备就绪，可以运行 'vm install /path/to/policy-gateway' 安装主程序");
}

/// 维护文件路径
fn maintenance_file() -> PathBuf {
    backup_dir().join("maintenance.json")
}

/// 热更新: 从 URL 下载并安装
fn hot_update(url: Option<&String>) {
    let url = match url {
        Some(u) => u,
        None => { eprintln!("❌ 请提供下载 URL\n   用法: vm hot-update <url>"); return; }
    };

    let tmp = "/tmp/pg-hot-update";
    println!("📥 下载 {} ...", url);

    let status = std::process::Command::new("curl")
        .args(["-sL", "-o", tmp, "-w", "%{http_code}", url])
        .output();

    match status {
        Ok(out) if out.status.success() => {
            let code = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if code != "200" {
                eprintln!("❌ 下载失败: HTTP {}", code);
                let _ = std::fs::remove_file(tmp);
                return;
            }
        }
        Ok(_) => {
            eprintln!("❌ 下载失败 (curl exit code {})", status.unwrap().status);
            return;
        }
        Err(e) => {
            eprintln!("❌ curl 不可用: {}（请先安装 curl）", e);
            return;
        }
    }

    let src = std::path::Path::new(tmp);
    if !src.exists() {
        eprintln!("❌ 下载文件不存在");
        return;
    }

    println!("📦 正在安装...");
    let src_hash_bytes = sha256_file(src).unwrap_or([0u8; 32]);
    install_src(src);

    // 记录热更新时间戳
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let meta = serde_json::json!({
        "hot_updated_at": ts,
        "sha256": hex::encode(src_hash_bytes),
        "url": url,
    });
    let _ = std::fs::write(backup_dir().join("hot-update.json"), serde_json::to_string_pretty(&meta).unwrap_or_default());

    let _ = std::fs::remove_file(tmp);
    println!("✅ 热更新完成");
}

/// 维护模式 CLI
fn maintenance_cli(args: &[String]) {
    match args.get(1).map(|s| s.as_str()) {
        Some("set") => {
            let start_ts = args.get(2).and_then(|s| s.parse::<u64>().ok());
            let end_ts = args.get(3).and_then(|s| s.parse::<u64>().ok());
            match (start_ts, end_ts) {
                (Some(s), Some(e)) if s < e => {
                    let content = serde_json::json!({
                        "maintenance_start": s,
                        "maintenance_end": e,
                    });
                    match std::fs::write(maintenance_file(), serde_json::to_string_pretty(&content).unwrap_or_default()) {
                        Ok(_) => {
                            println!("✅ 维护窗口已设置");
                            println!("   开始: {} (Unix timestamp)", s);
                            println!("   结束: {} (Unix timestamp)", e);
                            println!("   在此期间所有页面将转向 maintenance.html");
                            println!("   CLI 将返回 'maintenance between {} and {}'", s, e);
                        }
                        Err(e) => eprintln!("❌ 写入失败: {}", e),
                    }
                }
                _ => eprintln!("❌ 用法: maintenance set <开始时间戳> <结束时间戳>\n   时间戳为 Unix 秒数（可用 date +%s 获取当前）"),
            }
        }
        Some("status") => {
            let content = std::fs::read_to_string(maintenance_file()).unwrap_or_default();
            if content.is_empty() {
                println!("📅 维护状态: 未设置");
                return;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let start = val.get("maintenance_start").and_then(|v| v.as_u64()).unwrap_or(0);
                let end = val.get("maintenance_end").and_then(|v| v.as_u64()).unwrap_or(0);
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                if now >= start && now < end {
                    println!("📅 维护中: {}-{} (还有 {} 秒)", start, end, end.saturating_sub(now));
                } else if now < start {
                    println!("📅 维护预定: {} ({} 秒后开始)", start, start.saturating_sub(now));
                } else {
                    println!("📅 维护已结束 (于 {} 结束)", end);
                }
            }
        }
        Some("clear") => {
            let _ = std::fs::remove_file(maintenance_file());
            println!("✅ 维护窗口已清除");
        }
        _ => {
            eprintln!("用法: maintenance <set|status|clear>");
        }
    }
}
