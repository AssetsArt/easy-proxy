#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub mod acme;
mod commands;
mod config;
pub mod errors;
mod proxy;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Tries to check your configuration quite thoroughly.
    #[arg(short = 't', long, default_value_t = false)]
    test: bool,
    // Reload the configuration
    #[arg(short = 'r', long = "reload", default_value_t = false)]
    reload: bool,
}

fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();

    // parse the command line arguments
    let args = Args::parse();
    if args.test {
        commands::Commands::send_command("test");
        std::process::exit(0);
    }
    if args.reload {
        commands::Commands::send_command("reload");
        std::process::exit(0);
    }

    match config::runtime::initialize() {
        Ok(_) => {
            tracing::info!("Configuration initialized successfully");
        }
        Err(e) => {
            tracing::error!("Error: {:?}", e);
            std::process::exit(1);
        }
    }

    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| tracing::error!("Error: {:?}", e))
        .unwrap();
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
        // println!("{:#?}", config::store::get());
    });

    // start the commands listener
    std::thread::spawn(|| {
        commands::Commands::run();
    });

    // create a new proxy
    proxy::EasyProxy::new_proxy()
        .map_err(|e| tracing::error!("Error: {:?}", e))
        .unwrap()
        .run_forever();
}
