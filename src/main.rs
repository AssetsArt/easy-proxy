mod config;
pub mod errors;
mod proxy;

fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();
    match config::runtime::initialize() {
        Ok(_) => {
            tracing::info!("Configuration initialized successfully");
        }
        Err(e) => {
            tracing::error!("Error: {:?}", e);
            std::process::exit(1);
        }
    }
    match config::proxy::load() {
        Ok(_) => {
            tracing::info!("Proxy configuration loaded successfully");
        }
        Err(e) => {
            tracing::error!("Error: {:?}", e);
            std::process::exit(1);
        }
    }

    // create a new proxy
    proxy::EasyProxy::new_proxy()
        .map_err(|e| tracing::error!("Error: {:?}", e))
        .unwrap()
        .run_forever();
}
