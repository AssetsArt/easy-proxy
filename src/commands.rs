use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::process;

const SOCKET_PATH: &str = "/tmp/easy-proxy.sock";

#[derive(Debug, Serialize, Deserialize)]
pub struct Commands {
    pub message_type: String,
    pub message: String,
}

impl Commands {
    pub fn run() {
        // Remove the socket file if it exists
        if let Err(e) = std::fs::remove_file(SOCKET_PATH) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::error!("Error removing socket file {}: {:?}", SOCKET_PATH, e);
                std::process::exit(1);
            }
        }

        // Bind to the socket
        let listener = match UnixListener::bind(SOCKET_PATH) {
            Ok(listener) => {
                tracing::info!("Listening on {}", SOCKET_PATH);
                listener
            }
            Err(e) => {
                tracing::error!("Unable to bind to {}: {:?}", SOCKET_PATH, e);
                std::process::exit(1);
            }
        };

        // Accept connections in a loop
        for stream_result in listener.incoming() {
            match stream_result {
                Ok(stream) => {
                    handle_connection(stream);
                }
                Err(e) => {
                    tracing::error!("Error accepting connection: {:?}", e);
                }
            }
        }
    }

    pub fn send_command(command_str: &str) {
        // Connect to the main socket
        let mut stream = match UnixStream::connect(SOCKET_PATH) {
            Ok(stream) => stream,
            Err(e) => {
                tracing::error!("Unable to connect to {}: {:?}", SOCKET_PATH, e);
                std::process::exit(1);
            }
        };

        // Prepare the command to send
        let command = Commands {
            message_type: "command".to_string(),
            message: command_str.to_string(),
        };
        let command_json = match serde_json::to_string(&command) {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Error serializing command: {:?}", e);
                return;
            }
        };

        // Send the command
        if let Err(e) = stream.write_all(command_json.as_bytes()) {
            tracing::error!("Error sending command: {:?}", e);
            return;
        }

        // Read the response
        let mut buffer = [0; 1024];
        match stream.read(&mut buffer) {
            Ok(n) => {
                if n == 0 {
                    tracing::error!("Received empty response");
                    return;
                }
                if n >= buffer.len() {
                    tracing::error!("Response too large");
                    return;
                }
                let response_str = std::str::from_utf8(&buffer[..n]).unwrap_or_default().trim();
                let res_command: Commands = match serde_json::from_str(response_str) {
                    Ok(cmd) => cmd,
                    Err(e) => {
                        tracing::error!("Error deserializing response: {:?}", e);
                        return;
                    }
                };
                if res_command.message_type == "error" {
                    tracing::error!("{}", res_command.message);
                    process::exit(1);
                } else {
                    tracing::info!("{}", res_command.message);
                }
            }
            Err(e) => {
                tracing::error!("Error reading response: {:?}", e);
            }
        }
    }
}

fn handle_connection(mut stream: UnixStream) {
    let mut buffer = [0; 1024];
    match stream.read(&mut buffer) {
        Ok(n) => {
            if n == 0 {
                return;
            }
            let buffer = &buffer[..n];
            // Process the command
            if let Err(e) = process_command(&mut stream, buffer) {
                tracing::error!("Error processing command: {:?}", e);
            }
        }
        Err(e) => {
            tracing::error!("Error reading from stream: {:?}", e);
        }
    }
}

fn process_command(
    stream: &mut UnixStream,
    buffer: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut res_command = Commands {
        message_type: "response".to_string(),
        message: String::new(),
    };
    let command_str = std::str::from_utf8(buffer)?.trim();
    let command: Commands = serde_json::from_str(command_str)?;

    if command.message_type == "command" {
        match command.message.as_str() {
            "reload" => {
                handle_reload_command(stream, &mut res_command)?;
            }
            "test" => {
                handle_test_command(stream, &mut res_command)?;
            }
            _ => {
                tracing::info!("Received unknown command: {:?}", command.message);
            }
        }
    }
    Ok(())
}

fn handle_reload_command(
    stream: &mut UnixStream,
    res_command: &mut Commands,
) -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match crate::config::proxy::load().await {
            Ok(_) => {
                tracing::info!("Proxy configuration loaded successfully");
                res_command.message = "Proxy configuration loaded successfully".to_string();
            }
            Err(e) => {
                tracing::error!("Error loading proxy configuration: {:?}", e);
                res_command.message_type = "error".to_string();
                res_command.message = format!("Error: {:?}", e);
            }
        }
        // Send response
        let res_command_str = serde_json::to_string(&res_command)?;
        stream.write_all(res_command_str.as_bytes())?;
        stream.flush()?;
        Ok(())
    })
}

fn handle_test_command(
    stream: &mut UnixStream,
    res_command: &mut Commands,
) -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        match crate::config::proxy::read().await {
            Ok(c) => match crate::config::store::load(c).await {
                Ok(_) => {
                    tracing::info!("Proxy configuration tested successfully");
                    res_command.message = "Proxy configuration tested successfully".to_string();
                }
                Err(e) => {
                    tracing::error!("Error loading proxy configuration: {:?}", e);
                    res_command.message_type = "error".to_string();
                    res_command.message = format!("Error loading proxy configuration: {:?}", e);
                }
            },
            Err(e) => {
                tracing::error!("Error reading proxy configuration: {:?}", e);
                res_command.message_type = "error".to_string();
                res_command.message = format!("Error reading proxy configuration: {:?}", e);
            }
        }
        // Send response
        let res_command_str = serde_json::to_string(&res_command)?;
        stream.write_all(res_command_str.as_bytes())?;
        stream.flush()?;
        Ok(())
    })
}
