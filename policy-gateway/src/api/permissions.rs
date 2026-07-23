//! /permissions — 权限表页面（仅管理员可见，需 token 验证）

use axum::extract::{State, Query};
use axum::http::StatusCode;
use axum::response::Html;
use std::sync::Arc;
use std::collections::HashMap;

use crate::AppState;
use crate::api::manager::check_auth;

fn html_escape(s: &str) -> String {
    s.chars().map(|c| match c {
        '<' => "&lt;".into(),
        '>' => "&gt;".into(),
        '&' => "&amp;".into(),
        '"' => "&quot;".into(),
        '\'' => "&#x27;".into(),
        _ => c.to_string(),
    }).collect()
}

/// GET /permissions — 权限表
///   ?token=xxx  → 根管理员, 全部可见
///   ?sha256=xxx → 持证人, 只看自己
pub async fn handle_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");
    let sha256_param = params.get("sha256").map(|s| s.as_str()).unwrap_or("");

    if token.is_empty() && sha256_param.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let table = state.auth_table.read().await;
    let is_admin = !token.is_empty() && check_auth(token);

    let mut rows = String::new();
    let mut count = 0usize;

    if is_admin {
        for entry in table.iter() {
            append_row(&mut rows, entry);
            count += 1;
        }
    } else if let Ok(bytes) = hex::decode(sha256_param) {
        if bytes.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            if let Some(entry) = table.get(&key) {
                append_row(&mut rows, entry);
                count = 1;
            }
        }
    }

    let title = if is_admin { "📋 权限表 (全部)" } else { "📋 我的权限" };
    let html = format!(r#"<!DOCTYPE html>
<html lang="zh"><head><meta charset="UTF-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>权限表</title><style>
*{{box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,sans-serif;max-width:960px;margin:0 auto;padding:16px;background:#f5f5f7;color:#1d1d1f}}
.card{{background:#fff;border-radius:12px;padding:16px;margin:12px 0;box-shadow:0 1px 3px#0000001a}}
table{{width:100%;border-collapse:collapse;font-size:14px}}
td,th{{padding:8px;text-align:left;border-bottom:1px solid#e5e5ea}}
th{{font-size:12px;color:#86868b}}
code{{background:#e8e8ed;padding:2px 6px;border-radius:4px;font-size:12px}}
</style></head>
<body><div class="card"><h1>{title}</h1><p style="font-size:13px;color:#86868b">{count} 条目</p>
<table><tr><th>主机名</th><th>SHA256</th><th>权限</th><th>状态</th></tr>{rows}</table></div></body></html>"#);
    Ok(Html(html))
}

fn append_row(rows: &mut String, entry: &crate::auth::PermissionEntry) {
    let sha = hex::encode(entry.sha256);
    let hostname = html_escape(&entry.hostname);
    let bm = format!("{:x}", entry.bitmap);
    let st = format!("{:?}", entry.status);
    rows.push_str(&format!("<tr><td>{hostname}</td><td><code>{sha:.16}…</code></td><td><code>{bm}</code></td><td>{st}</td></tr>\n"));
}
