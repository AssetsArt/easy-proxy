// internal modules
pub mod api;
pub mod config;
pub mod db;
pub mod jwt;
pub mod proxy;

// external crates
use futures_util::future::join;
#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[tokio::main]
async fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();

    // initialize the database
    db::get_database().await;

    // start the api server
    let app_svc = async move { 
        // todo!("start the api server")
     };

    // start the proxy server
    let prox_svc = async move {
        match proxy::Proxy::new().listen().await {
            Ok(_) => {
                tracing::info!("Proxy server stopped");
            }
            Err(e) => {
                tracing::error!("Error: {}", e);
            }
        }
    };

    join(app_svc, prox_svc).await;
}
