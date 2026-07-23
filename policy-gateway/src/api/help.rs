//! /api/help — 设备接入教程（JSON，兼容所有 HTTP 设备）

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
        title: "MCU / 单片机接入 (推荐 pubkey 直发)".into(),
        steps: vec![
            HelpStep {
                title: "1. 生成密钥".into(),
                body: "Ed25519 密钥，只发公钥 hex（32 字节 = 64 hex 字符）\n比 CSR 简单 10 倍，单片机无压力".into(),
                cli: Some("# ESP32 (Arduino):\n  uint8_t pk[32];\n  crypto_sign_keypair(pk, sk);\n  Serial.printf(\"PUBKEY: %s\\n\", bin2hex(pk, 32));".into()),
                expected: Some("输出 64 hex 字符，如: 3b6a27bccef3c15e6a7c8b2d1a9f0e5d...".into()),
            },
            HelpStep {
                title: "2. 提交申请".into(),
                body: "POST 公钥到 /api/signup，一行 curl 搞定".into(),
                cli: Some("curl -X POST http://host:8443/api/signup \\\n  -H 'Content-Type: application/json' \\\n  -d '{\"pubkey\":\"<64_hex>\",\"hostname\":\"my-device\",\"requested\":\"01\"}'".into()),
                expected: Some("{\"request_id\":\"...\",\"status\":\"pending_confirm\"}".into()),
            },
            HelpStep {
                title: "3. 轮询状态".into(),
                body: "提交后返回 poll_interval=15，每 15 秒查一次即可\n管理员批准后 status→active，即代表可上网".into(),
                cli: None,
                expected: Some("{\"status\":\"active\"} → 可上网".into()),
            },
        ],
    }
}

fn browser_guide() -> HelpResponse {
    HelpResponse {
        topic: "browser".into(),
        title: "浏览器接入".into(),
        steps: vec![
            HelpStep {
                title: "1. 访问 /signup".into(),
                body: "打开 http://<gateway>:8443/signup\n页面引导生成 CSR 并提交".into(),
                cli: None,
                expected: Some("看到证书申请页面".into()),
            },
            HelpStep {
                title: "2. 填写表单".into(),
                body: "设备名 + 权限模板 + CSR（浏览器自动生成或粘贴）\n提交后等管理员审批".into(),
                cli: None,
                expected: Some("显示 request_id 和待审批状态".into()),
            },
            HelpStep {
                title: "3. 查看状态".into(),
                body: "刷新 /signup/status?id=<request_id>\nactive → 可上网".into(),
                cli: None,
                expected: None,
            },
        ],
    }
}

fn cli_guide() -> HelpResponse {
    HelpResponse {
        topic: "cli".into(),
        title: "命令行 / 脚本接入 (pg CLI)".into(),
        steps: vec![
            HelpStep {
                title: "1. 设置环境".into(),
                body: "PG_SERVER=http://gateway:8443  PG_TOKEN=<token>".into(),
                cli: Some("export PG_SERVER=http://gateway:8443\nexport PG_TOKEN=<manager-token>".into()),
                expected: None,
            },
            HelpStep {
                title: "2. 申请证书 (pubkey)".into(),
                body: "一行命令申请：pg cert sign \"<64_hex_pubkey>\"".into(),
                cli: Some("pg cert sign \"abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234abcd1234\"".into()),
                expected: Some("返回 request_id + pending_confirm".into()),
            },
            HelpStep {
                title: "3. 查看待审批".into(),
                body: "管理员查看待审批列表".into(),
                cli: Some("pg pending".into()),
                expected: Some("显示所有待审批条目 JSON".into()),
            },
            HelpStep {
                title: "4. 批准/驳回".into(),
                body: "管理员批准".into(),
                cli: Some("pg approve <request_id>\npg reject <request_id>".into()),
                expected: Some("{\"status\":\"approved\"}".into()),
            },
            HelpStep {
                title: "5. 查询状态".into(),
                body: "用 SHA256 查证书状态".into(),
                cli: Some("pg cert status <sha256_hex>".into()),
                expected: Some("{\"status\":\"active\"}".into()),
            },
        ],
    }
}

fn headless_guide() -> HelpResponse {
    HelpResponse {
        topic: "headless".into(),
        title: "无头设备 / Linux 服务器接入 (curl + openssl)".into(),
        steps: vec![
            HelpStep {
                title: "1. 生成 CSR".into(),
                body: "用 openssl 生成 Ed25519 密钥和 CSR".into(),
                cli: Some("openssl req -new -newkey ed25519 -nodes \\\n  -keyout /etc/ssl/device.key -out /tmp/device.csr \\\n  -subj \"/CN=$(hostname)\"".into()),
                expected: Some("device.key + device.csr".into()),
            },
            HelpStep {
                title: "2. 提交申请".into(),
                body: "POST CSR 到 /api/signup\n保存返回的 request_id 和 cert_pem".into(),
                cli: Some("CSR=$(cat /tmp/device.csr | tr '\\n' ' ')\nRESP=$(curl -s -X POST http://host:8443/api/signup \\\n  -H 'Content-Type: application/json' \\\n  -d \"{\\\"csr\\\":\\\"$CSR\\\",\\\"hostname\\\":\\\"$(hostname)\\\"}\")\nRID=$(echo $RESP | grep -o '\"request_id\":\"[^\"]*\"' | cut -d'\"' -f4)\nCERT=$(echo $RESP | grep -o '\"cert_pem\":\"[^\"]*\"' | cut -d'\"' -f4)".into()),
                expected: Some("RID + CERT 存好，等审批".into()),
            },
            HelpStep {
                title: "3. 定时续签 (可选)".into(),
                body: "cron 每周跑一次".into(),
                cli: Some("0 0 * * 0 /path/to/setup.sh # 证书过期前自动续".into()),
                expected: None,
            },
        ],
    }
}
