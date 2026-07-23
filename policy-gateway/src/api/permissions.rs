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

/// GET /permissions?token=xxx — HTML 表格（需管理员 Token）
pub async fn handle_page(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Html<String>, StatusCode> {
    let token = params.get("token").map(|s| s.as_str()).unwrap_or("");
    if !check_auth(token) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let table = state.auth_table.read().await;
    let entries = table.iter();

    let mut rows = String::new();
    for entry in entries {
        let sha256_hex = hex::encode(entry.sha256);
        let hostname = html_escape(&entry.hostname);
        let bitmap_hex = format!("{:x}", entry.bitmap);
        let status = format!("{:?}", entry.status);
        let hw = entry.hw_platform.as_deref().unwrap_or("—");
        let created = if entry.created_at > 0 {
            // 使用 chrono 格式化时间
            let dt = chrono::DateTime::from_timestamp(entry.created_at, 0)
                .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "?".into());
            dt
        } else { "?".into() };

        rows.push_str(&format!(
            "<tr><td><code>{sha256_hex:.12}…</code></td>\
             <td>{hostname}</td>\
             <td><code>{bitmap_hex}</code></td>\
             <td>{status}</td>\
             <td>{hw}</td>\
             <td>{created}</td></tr>\n"
        ));
    }

    let count = table.iter().count();
    let html = format!(r#"<!DOCTYPE html>
<html lang="zh">
<head><meta charset="UTF-8"><title>权限表</title>
<style>
body{{font-family:sans-serif;max-width:1000px;margin:auto;padding:20px}}
table{{width:100%;border-collapse:collapse}}
td,th{{border:1px solid #ddd;padding:6px;text-align:left;font-size:14px}}
tr:nth-child(even){{background:#f9f9f9}}
code{{font-size:13px;background:#eee;padding:1px 4px;border-radius:3px}}
</style></head>
<body>
<h1>📋 权限表 <span style="font-size:14px;color:#888">({count} 条目)</span></h1>
<table>
<tr><th>SHA256</th><th>主机名</th><th>位图</th><th>状态</th><th>平台</th><th>创建</th></tr>
{rows}
</table>
<p><a href="/manager" style="color:#06c">← 返回审批面板</a></p>
</body></html>"#);

    Ok(Html(html))
}
