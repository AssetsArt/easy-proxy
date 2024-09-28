mod config;
pub mod errors;
mod proxy;

use config::runtime::initialize;

fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();
    match initialize() {
        Ok(_) => {
            tracing::info!("Configuration initialized successfully");
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
