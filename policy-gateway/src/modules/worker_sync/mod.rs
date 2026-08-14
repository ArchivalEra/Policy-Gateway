#![allow(dead_code)]
//! worker-sync — 与 Cloudflare Worker 双向同步 (纯 TCP, 零额外依赖)

use crate::modules::{CoreState, GatewayModule, ModuleDeclaration};
use axum::Router;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Worker 同步会话
pub struct WorkerSync {
    worker_host: String,
    worker_token: String,
    interval_secs: u64,
    use_tls: bool,
    core: Option<Arc<CoreState>>,
}

impl WorkerSync {
    pub fn new(worker_url: &str, worker_token: &str, interval_secs: u64) -> Self {
        let use_tls = worker_url.starts_with("https://");
        let host = worker_url
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        log::info!(
            "📡 worker-sync: {} (每 {}s 同步, {})",
            host,
            interval_secs,
            if use_tls { "TLS" } else { "明文(仅测试)" }
        );
        Self { worker_host: host, worker_token: worker_token.to_string(), interval_secs, use_tls, core: None }
    }

    pub async fn run(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(self.interval_secs));
        loop {
            interval.tick().await;
            self.sync_once().await;
        }
    }

    async fn sync_once(&self) {
        let body = serde_json::json!({
            "token": self.worker_token,
            "node": "router",
            "events": [],
        });
        let body_str = serde_json::to_string(&body).unwrap_or_default();

        let request = format!(
            "POST /api/sync/push HTTP/1.1\r\n\
             Host: {}\r\n\
             X-Sync-Token: {}\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n\
             {}",
            self.worker_host, self.worker_token, body_str.len(), body_str
        );

        match self.raw_http(&request).await {
            Ok(status) if status == 200 || status == 204 => {
                log::debug!("📡 sync OK");
            }
            Ok(status) => log::warn!("📡 sync 返回 HTTP {}", status),
            Err(e) => log::warn!("📡 sync 失败: {}", e),
        }
    }

    async fn raw_http(&self, request: &str) -> Result<u16, String> {
        let (host, port) = if let Some(pos) = self.worker_host.find(':') {
            let h = &self.worker_host[..pos];
            let p = self.worker_host[pos + 1..].parse::<u16>().unwrap_or(443);
            (h.to_string(), p)
        } else {
            (self.worker_host.clone(), 443u16)
        };

        let tcp = tokio::net::TcpStream::connect(format!("{}:{}", host, port))
            .await
            .map_err(|e| format!("connect {}:{}: {}", host, port, e))?;

        if self.use_tls {
            // TLS 加密传输（https），令牌与事件不明文上路
            let mut roots = rustls::RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let config = rustls::ClientConfig::builder()
                .with_root_certificates(roots)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
            let server_name = rustls::pki_types::ServerName::try_from(host.clone())
                .map_err(|e| format!("invalid hostname {}: {}", host, e))?;
            let mut stream = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| format!("tls handshake {}: {}", host, e))?;
            let _ = stream.write_all(request.as_bytes()).await;
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            Ok(parse_http_status(&buf[..n]))
        } else {
            // 明文 TCP（仅 http:// 测试地址）
            let mut stream = tcp;
            let _ = stream.write_all(request.as_bytes()).await;
            let mut buf = vec![0u8; 4096];
            let n = stream.read(&mut buf).await.unwrap_or(0);
            Ok(parse_http_status(&buf[..n]))
        }
    }
}

/// 从原始 HTTP 响应中解析状态码（0 = 解析失败）
fn parse_http_status(resp: &[u8]) -> u16 {
    String::from_utf8_lossy(resp)
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0)
}

impl GatewayModule for WorkerSync {
    fn name(&self) -> &'static str { "worker-sync" }
    fn declaration(&self) -> ModuleDeclaration {
        ModuleDeclaration {
            name: "worker-sync", version: 1, claim_bits: vec![],
            min_core_version: "0.3.3",
        }
    }
    fn mount(&self, _state: CoreState) -> Router { Router::new() }
}
