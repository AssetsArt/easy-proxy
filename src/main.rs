// internal modules
pub mod api;
pub mod config;
pub mod db;
pub mod jwt;
pub mod proxy;

#[tokio::main]
async fn main() {
    // initialize the database
    db::get_database().await;
    // start the api server
    tokio::spawn(async move {
        api::start().await;
    });
    match proxy::server::listener().await {
        Ok(_) => println!("Proxy server stopped"),
        Err(e) => eprintln!("Error: {}", e),
    }
}
