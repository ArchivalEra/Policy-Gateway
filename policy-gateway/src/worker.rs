//! Cloudflare Worker — 公网节点
//!
//! Phase 0 提供与路由器相同的 API 端点：
//!   POST /api/signup
//!   GET  /api/signup/status
//!   POST /api/manager/approve
//!   GET  /sync/pull
//!   POST /sync/push
//!
//! 使用 Cloudflare KV 存储权限表，实现多区域一致。

pub fn worker_routes() -> String {
    // 此文件是 Worker 逻辑的 Rust 接口
    // 实际 Worker 部署用独立的 JavaScript 文件（见 worker/ 目录）
    "Worker 逻辑 — 部署为独立 JS".into()
}
