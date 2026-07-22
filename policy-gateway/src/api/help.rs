//! /api/help — 设备接入教程（JSON 格式，MCU/浏览器通用）
//!
//! GET  /api/help           → JSON 教程清单
//! GET  /api/help?topic=mcu → MCU 专用教程

use axum::extract::Query;
use axum::Json;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct HelpResponse {
    pub topic: String,
    pub title: String,
    pub steps: Vec<HelpStep>,
}

#[derive(Serialize)]
pub struct HelpStep {
    pub title: String,
    pub body: String,
    pub cli: Option<String>,
    pub expected: Option<String>,
}

pub async fn handle(
    Query(q): Query<HashMap<String, String>>,
) -> Json<Vec<HelpResponse>> {
    let topic = q.get("topic").map(|s| s.as_str()).unwrap_or("all");
    match topic {
        "mcu" => Json(vec![mcu_guide()]),
        "browser" => Json(vec![browser_guide()]),
        "cli" => Json(vec![cli_guide()]),
        "headless" => Json(vec![headless_guide()]),
        _ => Json(vec![mcu_guide(), browser_guide(), cli_guide(), headless_guide()]),
    }
}

fn mcu_guide() -> HelpResponse {
    HelpResponse {
        topic: "mcu".into(),
        title: "MCU / 单片机接入指南".into(),
        steps: vec![
            HelpStep {
                title: "1. 生成密钥对".into(),
                body: "如果 MCU 支持 TLS（如 ESP32 带 mbedTLS），用硬件 CSR 生成。\n否则手动生成：openssl req -new -newkey rsa:2048 -nodes -keyout device.key -out device.csr -subj \"/CN=my-device\"".into(),
                cli: Some("openssl req -new -newkey rsa:2048 -nodes -keyout device.key -out device.csr -subj \"/CN=my-device\"".into()),
                expected: Some("生成 device.key 和 device.csr 两个文件".to_string()),
            },
            HelpStep {
                title: "2. 提交申请".into(),
                body: "将 CSR 内容 POST 到 /api/signup\ncurl -X POST http://<gateway>:8443/api/signup -H 'Content-Type: application/json' -d '{\"csr\":\"<PEM_CSR>\",\"hostname\":\"device-name\",\"requested\":\"01\"}'".into(),
                cli: Some("CSR=$(cat device.csr | tr '\\n' ' ')\ncurl -X POST http://gateway:8443/api/signup \\\n  -H 'Content-Type: application/json' \\\n  -d \"{\\\"csr\\\":\\\"$CSR\\\",\\\"hostname\\\":\\\"esp32-sensor\\\",\\\"requested\\\":\\\"01\\\"}\"".into()),
                expected: Some("返回 JSON: {\"request_id\":\"...\",\"cert_pem\":\"...PEM...\"}".to_string()),
            },
            HelpStep {
                title: "3. 确认证书".into(),
                body: "收到证书后，立即 POST /api/cert-confirm 确认。\ncurl -X POST http://<gateway>:8443/api/cert-confirm -H 'Content-Type: application/json' -d '{\"request_id\":\"<ID>\",\"token\":\"<MANAGER_TOKEN>\"}'".into(),
                cli: Some("curl -X POST http://gateway:8443/api/cert-confirm -H 'Content-Type: application/json' -d '{\"request_id\":\"<ID>\",\"token\":\"<TOKEN>\"}'".into()),
                expected: Some("返回 {\"status\":\"ok\"}".to_string()),
            },
            HelpStep {
                title: "4. 保存证书".into(),
                body: "将返回的 cert_pem 保存到 MCU 的文件系统或 flash。\n设备发起 HTTPS 请求时携带此证书 → 路由器放行。".into(),
                cli: None,
                expected: Some("证书存到 /certs/device.pem，mTLS 握手后用证书访问互联网".to_string()),
            },
            HelpStep {
                title: "5. MCU 无 TLS 能力 — pubkey 直发（推荐）".into(),
                body: "MCU 只需发送公钥 hex（比 CSR 简单 10 倍）：\ncurl -X POST http://host:8443/api/signup -H 'Content-Type: application/json' -d '{\"pubkey\":\"<32_byte_ed25519_hex>\",\"hostname\":\"esp32-sensor\"}'\n服务器自动包装为证书 + CA 签名。".into(),
                cli: Some("# ESP32 (Arduino): 生成 Ed25519 密钥对，输出公钥 hex\n# curl -X POST http://host:8443/api/signup \\\n#   -H 'Content-Type: application/json' \\\n#   -d '{\"pubkey\":\"<公钥hex>\",\"hostname\":\"esp-sensor\",\"hw_platform\":\"esp32\"}'".into()),
                expected: Some("返回 pending_confirm + 签名证书 PEM".to_string()),
            },
        ],
    }
}

fn browser_guide() -> HelpResponse {
    HelpResponse {
        topic: "browser".into(),
        title: "浏览器接入指南".into(),
        steps: vec![
            HelpStep {
                title: "1. 访问 /signup".into(),
                body: "在浏览器打开 http://<gateway>:8443/signup\n如果还没有证书，页面会引导你生成。".into(),
                cli: None,
                expected: Some("看到证书申请页面".to_string()),
            },
            HelpStep {
                title: "2. 填写主机名 + 权限模板".into(),
                body: "选择模板: connector（上网）/ device（计算节点）\n提交后等待管理员审批。".into(),
                cli: None,
                expected: Some("看到申请提交成功".to_string()),
            },
            HelpStep {
                title: "3. 管理员批准后确认".into(),
                body: "刷新 /signup/status?id=<request_id> 查看状态。\n变为 active 后即可上网。".into(),
                cli: None,
                expected: Some("状态变为 active".to_string()),
            },
        ],
    }
}

fn cli_guide() -> HelpResponse {
    HelpResponse {
        topic: "cli".into(),
        title: "命令行工具指南".into(),
        steps: vec![
            HelpStep {
                title: "policy-gateway 命令".into(),
                body: "  perm gc       — 清理过期权限\n  perm list     — 列出所有权限\n  perm stats    — 统计信息\n  vm snapshot   — 创建快照\n  vm rollback   — 回滚\n  init          — 首次设置\n  --help        — 帮助".into(),
                cli: None,
                expected: None,
            },
            HelpStep {
                title: "MCU 证书申请".into(),
                body: "方法1 (推荐): 直接发送公钥 hex\n  PUBKEY=$(esp32_generate_key | grep pubkey | cut -d' ' -f2)\n  curl -X POST http://host:8443/api/signup \\\n    -H 'Content-Type: application/json' \\\n    -d '{\"pubkey\":\"$PUBKEY\",\"hostname\":\"sensor1\"}'\n\n方法2 (标准): 生成 CSR 后提交\n  openssl req -new -newkey rsa:2048 -nodes -keyout key.pem -out csr.pem\n  CSR=$(cat csr.pem)\n  curl -X POST http://host:8443/api/signup \\\n    -H 'Content-Type: application/json' \\\n    -d '{\"csr\":\"$CSR\",\"hostname\":\"sensor1\"}'".into(),
                cli: None,
                expected: None,
            },
            HelpStep {
                title: "policy-gateway-vm 命令".into(),
                body: "  install <path> — 安装主程序\n  snapshot [name] — 快照\n  rollback [id]  — 回滚\n  list           — 列举快照\n  status         — 状态\n  verify         — 校验完整性\n  init           — 初始化目录".into(),
                cli: None,
                expected: None,
            },
        ],
    }
}

fn headless_guide() -> HelpResponse {
    HelpResponse {
        topic: "headless".into(),
        title: "无头设备 / 服务器接入指南".into(),
        steps: vec![
            HelpStep {
                title: "1. 自动脚本".into(),
                body: "将以下内容保存为 setup.sh 并运行：\n```bash\n#!/bin/sh\nHOST=\"gateway:8443\"\nNAME=$(hostname)\nopenssl req -new -newkey rsa:2048 -nodes -keyout /etc/ssl/device.key -out /tmp/device.csr -subj \"/CN=$NAME\"\nCSR=$(cat /tmp/device.csr)\nRESP=$(curl -s -X POST http://$HOST/api/signup -H 'Content-Type: application/json' -d \"{\\\"csr\\\":\\\"$CSR\\\",\\\"hostname\\\":\\\"$NAME\\\"}\")\nRID=$(echo $RESP | grep -o '\"request_id\":\"[^\"]*\"' | cut -d'\"' -f4)\nCERT=$(echo $RESP | grep -o '\"cert_pem\":\"[^\"]*\"' | cut -d'\"' -f4)\necho \"$CERT\" > /etc/ssl/device.crt\ncurl -s -X POST http://$HOST/api/cert-confirm -d \"{\\\"request_id\\\":\\\"$RID\\\"}\"\n```".into(),
                cli: None,
                expected: Some("执行后 /etc/ssl/device.crt 就是有效证书".to_string()),
            },
            HelpStep {
                title: "2. 定时续签（可选）".into(),
                body: "添加 cron: 0 0 * * 0 /path/to/setup.sh\n证书过期前自动重新申请。\n注意: 续签需要在旧证书过期前完成。".into(),
                cli: None,
                expected: None,
            },
            HelpStep {
                title: "3. 检查状态".into(),
                body: "curl http://<gateway>:8443/api/signup/status?sha256=<hex>\n返回 status 字段: pending / active / rejected / compromised".into(),
                cli: None,
                expected: Some("{\"status\":\"active\",\"hostname\":\"...\"}".to_string()),
            },
        ],
    }
}
