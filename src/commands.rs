use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{Read, Write};

#[derive(Debug, Serialize, Deserialize)]
pub struct Commands {
    pub message_type: String,
    pub message: String,
}

impl Commands {
    pub fn run() {
        let _ = std::fs::remove_file("/tmp/easy-proxy.sock");
        let listener = match std::os::unix::net::UnixListener::bind("/tmp/easy-proxy.sock") {
            Ok(listener) => {
                tracing::info!("Listening on /tmp/easy-proxy.sock");
                listener
            }
            Err(e) => {
                tracing::error!("Unable to bind to /tmp/easy-proxy.sock: {:?}", e);
                std::process::exit(1);
            }
        };
        while let Ok((mut stream, _)) = listener.accept() {
            let mut buffer = [0; 1024];
            match stream.read(&mut buffer) {
                Ok(n) => {
                    if n == 0 {
                        continue;
                    }
                    let buffer: Vec<u8> = buffer[..n].to_vec();
                    let mut stream_resc = match std::os::unix::net::UnixStream::connect(
                        "/tmp/easy-proxy-resc.sock",
                    ) {
                        Ok(stream_resc) => stream_resc,
                        Err(_) => continue,
                    };
                    let mut res_command = Commands {
                        message_type: "response".to_string(),
                        message: "".to_string(),
                    };
                    let command = std::str::from_utf8(&buffer).unwrap_or_default().trim();
                    // println!("{:?}", command);
                    let command: Commands = match serde_json::from_str(command) {
                        Ok(command) => command,
                        Err(e) => {
                            tracing::error!("Error deserializing command: {:?}", e);
                            continue;
                        }
                    };
                    // println!("{:#?}", command);
                    if command.message_type == *"command" {
                        if command.message == *"reload" {
                            let rt = match tokio::runtime::Runtime::new() {
                                Ok(rt) => rt,
                                Err(e) => {
                                    tracing::error!("Error creating runtime: {:?}", e);
                                    continue;
                                }
                            };
                            rt.block_on(async {
                                match crate::config::proxy::load().await {
                                    Ok(_) => {
                                        tracing::info!("Proxy configuration loaded successfully");
                                        res_command.message =
                                            "Proxy configuration loaded successfully".to_string();
                                        match serde_json::to_string(&res_command) {
                                            Ok(res_command) => {
                                                match stream_resc.write_all(res_command.as_bytes())
                                                {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Error sending response: {:?}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Error serializing response: {:?}",
                                                    e
                                                );
                                            }
                                        };
                                    }
                                    Err(e) => {
                                        tracing::error!("Error: {:?}", e);
                                        res_command.message_type = "error".to_string();
                                        res_command.message = format!("Error: {:?}", e);
                                        match serde_json::to_string(&res_command) {
                                            Ok(res_command) => {
                                                match stream_resc.write_all(res_command.as_bytes())
                                                {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Error sending response: {:?}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Error serializing response: {:?}",
                                                    e
                                                );
                                            }
                                        };
                                    }
                                }
                            });
                        } else if command.message == *"test" {
                            let rt = match tokio::runtime::Runtime::new() {
                                Ok(rt) => rt,
                                Err(e) => {
                                    tracing::error!("Error creating runtime: {:?}", e);
                                    continue;
                                }
                            };
                            rt.block_on(async {
                                match crate::config::proxy::read().await {
                                    Ok(_) => {
                                        tracing::info!("Proxy configuration test successful");
                                        res_command.message =
                                            "Proxy configuration test successful".to_string();
                                        match serde_json::to_string(&res_command) {
                                            Ok(res_command) => {
                                                match stream_resc.write_all(res_command.as_bytes())
                                                {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Error sending response: {:?}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Error serializing response: {:?}",
                                                    e
                                                );
                                            }
                                        };
                                    }
                                    Err(e) => {
                                        tracing::error!("Error: {:?}", e);
                                        res_command.message_type = "error".to_string();
                                        res_command.message = format!("Error: {:?}", e);
                                        match serde_json::to_string(&res_command) {
                                            Ok(res_command) => {
                                                match stream_resc.write_all(res_command.as_bytes())
                                                {
                                                    Ok(_) => {}
                                                    Err(e) => {
                                                        tracing::error!(
                                                            "Error sending response: {:?}",
                                                            e
                                                        );
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Error serializing response: {:?}",
                                                    e
                                                );
                                            }
                                        };
                                    }
                                }
                            });
                        } else {
                            tracing::info!("Received unknown command: {:?}", command);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error reading from stream: {:?}", e);
                }
            }
        }
    }

    pub fn send_command(command: &str) {
        let _ = std::fs::remove_file("/tmp/easy-proxy-resc.sock");
        let listener = match std::os::unix::net::UnixListener::bind("/tmp/easy-proxy-resc.sock") {
            Ok(listener) => listener,
            Err(e) => {
                tracing::error!("Unable to bind to /tmp/easy-proxy-resc.sock: {:?}", e);
                std::process::exit(1);
            }
        };
        std::thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0; 1024];
                match stream.read(&mut buffer) {
                    Ok(n) => {
                        if n == 0 {
                            continue;
                        }
                        // println!("{:?}", buffer);
                        let buffer: Vec<u8> = buffer[..n].to_vec();
                        let data = std::str::from_utf8(&buffer).unwrap_or_default().trim();
                        let command: Commands = match serde_json::from_str(data) {
                            Ok(command) => command,
                            Err(e) => {
                                tracing::error!("Error deserializing command: {:?}", e);
                                continue;
                            }
                        };

                        if command.message_type == "error" {
                            tracing::error!("{}", command.message);
                        } else {
                            tracing::info!("{}", command.message);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error reading from stream: {:?}", e);
                    }
                }
            }
        });
        let mut stream = match std::os::unix::net::UnixStream::connect("/tmp/easy-proxy.sock") {
            Ok(stream) => stream,
            Err(e) => {
                tracing::error!("Unable to connect to /tmp/easy-proxy.sock: {:?}", e);
                std::process::exit(1);
            }
        };
        let command = json!({
            "message_type": "command",
            "message": command,
        });
        let command = match serde_json::to_string(&command) {
            Ok(command) => command,
            Err(e) => {
                tracing::error!("Error serializing command: {:?}", e);
                return;
            }
        };
        match stream.write_all(command.as_bytes()) {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("Error sending command: {:?}", e);
            }
        }
        // sleep for a bit to allow the command to be processed
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
