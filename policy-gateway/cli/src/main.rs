//! pg — policy-gateway CLI (平台无关)
//!
//! 用法:
//!   pg auth login <server>         登录到服务器
//!   pg auth logout                 退出
//!   pg cert sign <csr> [--hostname <n>]  提交证书申请
//!   pg cert status <sha256>        查询证书状态
//!   pg approve <request_id>        批准申请
//!   pg reject <request_id>         拒绝申请
//!   pg pending                     列出待审批
//!   pg permissions                 列出权限表
//!   pg status                      服务器状态

use std::collections::HashMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 { print_help(); return; }

    match args[1].as_str() {
        "auth" => cmd_auth(&args[2..]),
        "cert" => cmd_cert(&args[2..]),
        "approve" | "reject" => cmd_approve(&args),
        "pending" => cmd_pending(&args),
        "permissions" => cmd_permissions(&args),
        "status" => cmd_status(&args),
        _ => print_help(),
    }
}

fn print_help() {
    println!("pg — policy-gateway CLI v{}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("用法:");
    println!("  pg auth login <server> [--token <t>]");
    println!("  pg auth logout");
    println!("  pg cert sign <csr_file|pubkey_hex> [--hostname <n>]");
    println!("  pg cert status <sha256>");
    println!("  pg approve <request_id>");
    println!("  pg reject <request_id>");
    println!("  pg pending");
    println!("  pg permissions");
    println!("  pg status");
}

fn cmd_auth(args: &[String]) {
    if args.is_empty() { eprintln!("用法: pg auth login <server>"); return; }
    match args[0].as_str() {
        "login" => {
            let server = args.get(1).map(|s| s.as_str()).unwrap_or("http://localhost:8443");
            let token = /* read from stdin or arg */ String::new();
            // Store token to $HOME/.pg/token
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let dir = std::path::Path::new(&home).join(".pg");
            std::fs::create_dir_all(&dir).ok();
            // ...save server + token
            println!("✅ 登录成功: {}", server);
        }
        "logout" => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let token_path = std::path::Path::new(&home).join(".pg/token");
            let _ = std::fs::remove_file(&token_path);
            println!("✅ 已退出");
        }
        _ => eprintln!("未知: {}", args[0]),
    }
}

fn cmd_cert(args: &[String]) {
    // Read CSR from file or pubkey from arg
    if args.is_empty() { eprintln!("用法: pg cert sign <csr|pubkey>"); return; }
    let input = &args[0];
    let hostname = args.iter().position(|a| a == "--hostname")
        .and_then(|i| args.get(i+1))
        .map(|s| s.as_str())
        .unwrap_or("cli-device");

    // Determine if input is a file (CSR) or hex pubkey
    let is_pubkey = input.len() == 64 && input.chars().all(|c| c.is_ascii_hexdigit());

    let body = if is_pubkey {
        format!(r#"{{"pubkey":"{}","hostname":"{}"}}"#, input, hostname)
    } else {
        let csr = std::fs::read_to_string(input).unwrap_or_else(|_| input.clone());
        format!(r#"{{"csr":{},"hostname":"{}"}}"#, serde_json::to_string(&csr).unwrap(), hostname)
    };
    println!("{}", body);
    // TODO: POST to server
    println!("📝 提交到服务器后返回 request_id");
}

fn cmd_approve(args: &[String]) {
    if args.len() < 2 { eprintln!("用法: pg approve|reject <request_id>"); return; }
    let action = &args[0]; // "approve" or "reject"
    let rid = &args[1];
    println!("{} {}", if action == "approve" { "✅ 批准" } else { "❌ 拒绝" }, rid);
    // TODO: POST /api/manager/approve
}

fn cmd_pending(args: &[String]) {
    println!("📋 待审批列表:");
    // TODO: GET /api/manager/pending
}

fn cmd_permissions(args: &[String]) {
    println!("📋 权限表:");
    // TODO: GET /api/permissions (or equivalent JSON)
}

fn cmd_status(args: &[String]) {
    println!("📊 服务器状态:");
    // TODO: GET /healthz
}
