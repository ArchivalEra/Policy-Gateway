//! VM — 版本管理器（核心委派层）
//!
//! 实际的版本管理由独立二进制 `policy-gateway-vm` 执行。
//! 核心中的 VM 模块只是一个委派层，通过 CLI 调用外部 VM。
//! 这样 VM 独立于主程序，主程序崩了 VM 还能回滚。

/// 调用外部 VM 二进制执行命令
fn call_vm(args: &[&str]) -> std::process::Output {
    // VM 二进制路径：优先 USB 上的独立版，其次闪存版
    let vm_bin = if std::path::Path::new("/mnt/usb/policy-gateway-vm").exists() {
        "/mnt/usb/policy-gateway-vm"
    } else if std::path::Path::new("/usr/sbin/policy-gateway-vm").exists() {
        "/usr/sbin/policy-gateway-vm"
    } else {
        // 回退：尝试同目录下的 vm 二进制
        "/tmp/system/policy-gateway-vm"
    };
    
    if !std::path::Path::new(vm_bin).exists() {
        // VM 独立二进制不存在，用核心内置的简易版
        return std::process::Command::new("echo")
            .arg("no vm binary found — 核心快照功能可用，模块回滚需要 vm-mod")
            .output()
            .unwrap();
    }
    
    std::process::Command::new(vm_bin)
        .args(args)
        .output()
        .unwrap()
}

/// CLI 入口 — 委派给外部 VM 或输出提示
pub fn cli(args: &[String]) {
    let output = call_vm(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
    print!("{}", String::from_utf8_lossy(&output.stdout));
    if !output.status.success() {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
    }
}
