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

    let conf = config::runtime::config();
    let http = &conf.proxy.http;
    let https = &conf.proxy.https;

    // check if the proxy is running
    match std::net::TcpListener::bind(http) {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("An error occurred while trying to bind to {}: {}", http, e);
            std::process::exit(1);
        }
    };
    match https {
        Some(https) => match std::net::TcpListener::bind(https) {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("An error occurred while trying to bind to {}: {}", https, e);
                std::process::exit(1);
            }
        },
        None => {}
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
