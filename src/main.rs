// internal modules
pub mod app;
pub mod config;
pub mod db;
pub mod jwt;
pub mod proxy;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    // initialize the database
    db::get_database().await;
    // start the api server
    tokio::spawn(async move {
        app::start().await;
    });
    match proxy::serve().await {
        Ok(_) => {
            tracing::info!("Proxy server stopped");
        }
        Err(e) => {
            tracing::error!("Error: {}", e);
        }
    }
}
