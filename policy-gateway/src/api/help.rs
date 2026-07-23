//! /api/help — 设备接入教程（JSON，MCU 优先）

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
        _ => Json(vec![mcu_guide()]),
    }
}

fn mcu_guide() -> HelpResponse {
    HelpResponse {
        topic: "mcu".into(),
        title: "MCU / 单片机接入 (推荐 pubkey 方式)".into(),
        steps: vec![
            HelpStep {
                title: "1. 生成密钥".into(),
                body: "Ed25519 密钥，只发公钥 hex 给服务器（32 字节 = 64 hex 字符）".into(),
                cli: Some("openssl genpkey -algorithm ed25519 -out /tmp/device.key\nopenssl pkey -in /tmp/device.key -pubout | tail -1 | xxd -r -p | xxd -p | tr -d '\\n'".into()),
                expected: Some("输出 64 个 hex 字符，例如: 3b6a27bccef3c15e6a7c8b2d1a9f0e5d4c3b2a1f0e5d4c3b2a1f0e5d4c3b2a".to_string()),
            },
            HelpStep {
                title: "2. 提交申请".into(),
                body: "POST 公钥到 /api/signup，一行 curl 搞定".into(),
                cli: Some("curl -X POST http://host:8443/api/signup \\\n  -H 'Content-Type: application/json' \\\n  -d '{\"pubkey\":\"<64_hex>\",\"hostname\":\"my-device\",\"requested\":\"01\"}'".into()),
                expected: Some("返回 {\"request_id\":\"...\",\"status\":\"pending_confirm\"}".to_string()),
            },
            HelpStep {
                title: "3. 等待审批".into(),
                body: "管理员同意后，证书自动生效。可通过 /api/signup/status?sha256=<hex> 查询状态。".into(),
                cli: Some("# 用返回的 sha256 查询状态\ncurl http://host:8443/api/signup/status?sha256=<sha256>".into()),
                expected: Some("{\"status\":\"active\"} → 成功，可以上网".to_string()),
            },
            HelpStep {
                title: "4. ESP32 快速集成".into(),
                body: "Arduino 示例:\n1. 生成 Ed25519 密钥\n2. 发送 pubkey hex\n3. 存储返回的证书到 NVS\n4. HTTPS 请求时携带证书".into(),
                cli: None,
                expected: Some("不需要 mTLS 库，SHA256 查表验证".to_string()),
            },
        ],
    }
}
