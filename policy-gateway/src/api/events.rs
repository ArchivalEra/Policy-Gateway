//! GET /api/events — 服务端推送事件 (SSE)
//!
//! MCU/浏览器连接此端点后，服务器实时推送证书状态变更。
//! 不需要轮询，纯事件驱动。
//!
//! 用法:
//!   curl -N http://host:8443/api/events
//!   → data: {"type":"approved","sha256":"...","hostname":"..."}
//!
//!   ESP32:
//!   esp_http_client_config_t cfg = { .url = "http://host:8443/api/events" };
//!   // 读取行，解析 data: 前缀

use axum::extract::State;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use futures::stream::StreamExt;
use std::sync::Arc;
use std::convert::Infallible;
use tokio_stream::wrappers::BroadcastStream;

use crate::AppState;

pub async fn handle(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let rx = state.event_tx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(msg) => futures::future::ready(Some(Ok::<_, Infallible>(Event::default().data(msg)))),
            Err(_) => futures::future::ready(None),
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(30))
            .text("keepalive"),
    )
}
