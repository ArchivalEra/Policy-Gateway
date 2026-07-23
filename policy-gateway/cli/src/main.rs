//! pg — policy-gateway 命令行客户端
//!
//! 纯 HTTP 客户端，零依赖（只用 std）
//! 用法:
//!   pg status                    健康检查
//!   pg approve <id>              批准
//!   pg reject <id>               拒绝
//!   pg pending                   待审批列表
//!   pg cert sign <csr|pubkey>    申请
//!   pg cert status <sha256>      状态

use std::io::{Read, Write};
use std::net::TcpStream;

fn get_server() -> (String, u16) {
    let s = std::env::var("PG_SERVER").unwrap_or_else(|_| "http://localhost:8443".to_string());
    let s = s.trim_start_matches("http://").trim_start_matches("https://");
    if let Some((host, port)) = s.split_once(':') {
        (host.to_string(), port.parse().unwrap_or(8443))
    } else {
        (s.to_string(), 8443)
    }
}

fn get_token() -> String {
    std::env::var("PG_TOKEN").unwrap_or_default()
}

fn http(method: &str, path: &str, body: Option<&str>) -> Result<String, String> {
    let (host, port) = get_server();
    let addr = format!("{}:{}", host, port);
    let mut stream = TcpStream::connect(&addr).map_err(|e| format!("connect: {}", e))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .map_err(|e| format!("timeout: {}", e))?;

    let (body, content_len) = match body {
        Some(b) => (b.to_string(), format!("Content-Length: {}\r\n", b.len())),
        None => (String::new(), String::new()),
    };

    let content_type = if body.is_empty() { String::new() } else { "Content-Type: application/json\r\n".to_string() };

    let request = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n{}Accept: application/json\r\n{}\r\n{}",
        method, path, host, content_type, content_len, body
    );

    stream.write_all(request.as_bytes()).map_err(|e| format!("write: {}", e))?;

    let mut buf = vec![0u8; 8192];
    let n = stream.read(&mut buf).map_err(|e| format!("read: {}", e))?;
    let response = String::from_utf8_lossy(&buf[..n]).to_string();

    // Extract body after \r\n\r\n
    if let Some(body_start) = response.find("\r\n\r\n") {
        Ok(response[body_start + 4..].trim().to_string())
    } else {
        Err(format!("no body in response: {}", response.lines().next().unwrap_or("?")))
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 { print_help(); return; }

    match args[1].as_str() {
        "status" | "health" => cmd_status(),
        "approve" if args.len() > 2 => cmd_approve(&args[2], "approve"),
        "reject" if args.len() > 2 => cmd_approve(&args[2], "reject"),
        "pending" | "list" => cmd_pending(),
        "cert" if args.len() > 2 => match args[2].as_str() {
            "sign" if args.len() > 3 => cmd_sign(&args[3]),
            "status" if args.len() > 3 => cmd_cert_status(&args[3]),
            _ => { eprintln!("用法: pg cert sign <csr|pubkey> | status <sha256>"); }
        },
        "help" | "--help" | "-h" => print_help(),
        _ => {
            eprintln!("pg: '{}' 不是 pg 指令。", args[1]);
            eprintln!("   核心指令: status, approve, reject, pending, cert sign, cert status");
            eprintln!("   模块指令: 安装对应模块后通过 vm-mod list 查看");
            eprintln!("   全部帮助: pg help");
        }
    }
}

fn print_help() {
    println!("pg — policy-gateway CLI (核心指令集, 永不变)");
    println!();
    println!("核心指令:");
    println!("  pg status                     服务器健康检查");
    println!("  pg approve <request_id>       批准申请");
    println!("  pg reject <request_id>        拒绝申请");
    println!("  pg pending                    待审批列表");
    println!("  pg cert sign <csr|pubkey>     提交证书申请");
    println!("  pg cert status <sha256>       查询证书状态");
    println!();
    println!("环境变量:");
    println!("  PG_SERVER     服务器地址 (默认: http://localhost:8443)");
    println!("  PG_TOKEN      管理 Token");
    println!("  PG_HOSTNAME   主机名 (cert sign 时使用)");
    println!();
    println!("模块指令: 通过 vm-mod install 安装对应模块后可用");
    println!("  运行 'vm-mod list' 查看已注册的模块指令");
}

fn cmd_status() {
    match http("GET", "/healthz", None) {
        Ok(r) => println!("{}", r),
        Err(e) => eprintln!("错误: {}", e),
    }
}

fn cmd_approve(request_id: &str, action: &str) {
    let token = get_token();
    if token.is_empty() { eprintln!("错误: PG_TOKEN 未设置"); return; }
    let body = format!(r#"{{"request_id":"{}","action":"{}","token":"{}","bitmap":1}}"#,
        request_id, action, token);
    match http("POST", "/api/manager/approve", Some(&body)) {
        Ok(r) => println!("{}", r),
        Err(e) => eprintln!("错误: {}", e),
    }
}

fn cmd_pending() {
    let token = get_token();
    if token.is_empty() { eprintln!("错误: PG_TOKEN 未设置"); return; }
    match http("GET", &format!("/api/manager/pending?token={}", token), None) {
        Ok(r) => println!("{}", r),
        Err(e) => eprintln!("错误: {}", e),
    }
}

fn cmd_sign(input: &str) {
    let is_pubkey = (input.len() == 64 || input.len() == 66) && input.chars().all(|c| c.is_ascii_hexdigit());
    let hostname = std::env::var("PG_HOSTNAME").unwrap_or_else(|_| "cli-device".to_string());
    let body = if is_pubkey {
        format!(r#"{{"pubkey":"{}","hostname":"{}","requested":"01"}}"#, input, hostname)
    } else {
        let csr = std::fs::read_to_string(input).unwrap_or_else(|_| input.to_string());
        // Escape for JSON: \ → \\, " → \", \n → \\n, \t → \\t
        let escaped = csr
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");
        format!(r#"{{"csr":"{}","hostname":"{}","requested":"01"}}"#, escaped, hostname)
    };
    match http("POST", "/api/signup", Some(&body)) {
        Ok(r) => println!("{}", r),
        Err(e) => eprintln!("错误: {}", e),
    }
}

fn cmd_cert_status(sha256: &str) {
    match http("GET", &format!("/api/signup/status?sha256={}", sha256), None) {
        Ok(r) => println!("{}", r),
        Err(e) => eprintln!("错误: {}", e),
    }
}
