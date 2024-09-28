mod commands;
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

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        match config::proxy::load().await {
            Ok(_) => {
                tracing::info!("Proxy configuration loaded successfully");
            }
            Err(e) => {
                tracing::error!("Error: {:?}", e);
                std::process::exit(1);
            }
        }
        println!("{:#?}", config::store::get());
    });
    // create a new proxy
    proxy::EasyProxy::new_proxy()
        .map_err(|e| tracing::error!("Error: {:?}", e))
        .unwrap()
        .run_forever();
}
