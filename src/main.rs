#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

mod acme;
mod commands;
mod config;
mod errors;
mod proxy;
mod utils;

use clap::Parser;
use commands::Commands;

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Tries to check your configuration thoroughly.
    #[arg(short, long, default_value_t = false)]
    test: bool,

    /// Reload the configuration.
    #[arg(short, long, default_value_t = false)]
    reload: bool,
}

fn main() {
    // Initialize the logger.
    tracing_subscriber::fmt::init();

    // Parse command-line arguments.
    let args = Args::parse();

    if args.test {
        Commands::send_command("test");
        std::process::exit(0);
    }

    if args.reload {
        Commands::send_command("reload");
        std::process::exit(0);
    }

    // Initialize configuration.
    if let Err(e) = config::runtime::initialize() {
        tracing::error!("Error initializing configuration: {:?}", e);
        std::process::exit(1);
    }
    tracing::info!("Configuration initialized successfully");

    // Load proxy configuration.
    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        if let Err(e) = config::proxy::load().await {
            tracing::error!("Error loading proxy configuration: {:?}", e);
            std::process::exit(1);
        }
        tracing::info!("Proxy configuration loaded successfully");
    });

    // Start the commands listener in a separate thread.
    std::thread::spawn(|| {
        Commands::run();
    });

    // Start the proxy server.
    proxy::EasyProxy::new_proxy()
        .expect("Failed to create proxy server")
        .run_forever();
}
