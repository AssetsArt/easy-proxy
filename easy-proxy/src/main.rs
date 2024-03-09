use std::os::unix::net::UnixStream;

#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use clap::Parser;
use std::io::{Read, Write};

/// Simple program to greet a person
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

    // parse the command line arguments
    let args = Args::parse();
    if args.test {
        match config::proxy::validate() {
            Ok(_) => {
                tracing::info!("✅ Proxy config is valid");
            }
            Err(e) => {
                tracing::error!("❌ {:?}", e);
                std::process::exit(1);
            }
        }
        // exit after testing the configuration
        std::process::exit(0);
    }

    if args.reload {
        match config::proxy::validate() {
            Ok(_) => {
                let mut stream = UnixStream::connect("/tmp/easy-proxy.sock")
                    .expect("Failed to connect to socket");
                match stream.write_all(b"reload") {
                    Ok(_) => {
                        tracing::info!("✅ Proxy config reloaded");
                    }
                    Err(e) => {
                        tracing::error!("❌ {:?}", e);
                    }
                }
                std::process::exit(0);
            }
            Err(e) => {
                tracing::error!("❌ {:?}", e);
                std::process::exit(1);
            }
        }
    }

    match config::proxy::initialize() {
        Ok(_) => {
            tracing::info!("✅ Proxy config initialized");
        }
        Err(e) => {
            tracing::error!("❌ Error initializing proxy config: {:?}", e);
            std::process::exit(1);
        }
    }

    // Open unix socket for reload command
    std::thread::spawn(|| {
        // remove the socket if it already exists
        let _ = std::fs::remove_file("/tmp/easy-proxy.sock");
        let listener = match std::os::unix::net::UnixListener::bind("/tmp/easy-proxy.sock") {
            Ok(listener) => {
                tracing::info!("✅ Listening on /tmp/easy-proxy.sock");
                listener
            }
            Err(e) => {
                tracing::error!("❌ {:?}", e);
                std::process::exit(1);
            }
        };
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0; 1024];
            // println!("Received a connection");
            match stream.read(&mut buffer) {
                Ok(_) => {
                    let command = std::str::from_utf8(&buffer).unwrap_or_default();
                    // println!("Command: {}", command);
                    if command.contains("reload") {
                        match config::proxy::reload() {
                            Ok(_) => {
                                tracing::info!("✅ Proxy config reloaded");
                            }
                            Err(e) => {
                                tracing::error!("❌ Error reloading proxy config: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("❌ {:?}", e);
                }
            }
        }
    });
    // create a new proxy
    proxy::Proxy::new_proxy()
        .map_err(|e| tracing::error!("❌ Error creating proxy: {:?}", e))
        .unwrap()
        .run_forever();
}
