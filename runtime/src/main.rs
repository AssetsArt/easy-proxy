#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();
    // initialize the config
    match config::runtime::initialize() {
        Ok(_) => {
            tracing::info!("✅ Config initialized");
        }
        Err(e) => {
            tracing::error!("❌ Error initializing config: {:?}", e);
            std::process::exit(1);
        }
    }
    config::proxy::read_config();

    // create a new proxy
    proxy::Proxy::new_proxy()
        .map_err(|e| tracing::error!("❌ Error creating proxy: {:?}", e))
        .unwrap()
        .run_forever();
}
