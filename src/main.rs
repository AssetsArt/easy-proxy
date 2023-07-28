// internal modules
pub mod api;
pub mod config;
pub mod proxy;
pub mod db;

#[tokio::main]
async fn main() {
    // initialize the database
    db::get_database().await;
    // start the api server
    tokio::spawn(async move {
        api::start().await;
    });
    proxy::server::start().await;
}