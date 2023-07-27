// internal modules
pub mod api;
pub mod config;
pub mod proxy;
pub mod server;
pub mod tokiort;

#[tokio::main]
async fn main() {
    server::start().await;
}
