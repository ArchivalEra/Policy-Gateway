mod tls;
mod auth;
mod api;
mod anti_abuse;
mod worker;

#[tokio::main]
async fn main() {
    println!("🔐 policy-gateway v0.1");
    println!("   mTLS 网关 + 权限表 + 计算调度");
    println!();
    println!("等待实现...");
}
