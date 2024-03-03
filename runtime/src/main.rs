use tracing;

#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();
    // initialize the config
    let _ = config::app_config();

    // create a new proxy
    proxy::Proxy::new()
        .map_err(|e| tracing::error!("Error starting proxy: {:?}", e))
        .unwrap()
        .run_forever();
}
