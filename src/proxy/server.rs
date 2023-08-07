use std::error::Error;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// external
use crate::config;

pub async fn listener() -> Result<(), Box<dyn Error>> {
    let config = config::load_global_config().clone();
    let addr = config.host.clone();
    let server_addr: std::net::SocketAddr = addr.parse()?;
    let listener = TcpListener::bind(server_addr).await?;

    println!("TCP proxy server listening on: {}", server_addr);
    // Accept incoming TCP connections
    while let Ok((client_stream, _)) = listener.accept().await {
        let socket_addr: SocketAddr = client_stream.peer_addr().unwrap();
        tokio::spawn(async move {
            if let Err(e) = handle_client(client_stream, socket_addr).await {
                eprintln!("Internal error: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_client(
    mut client_stream: TcpStream,
    _socket_addr: SocketAddr,
) -> Result<(), Box<dyn Error>> {
    let remote_server: SocketAddr = "127.0.0.1:3000".to_string().parse()?;
    let (mut client_reader, mut client_writer) = client_stream.split();
    let mut server_stream: Option<TcpStream> = None;
    // 4 MiB
    let max_request_size = 4 * 1024 * 1024;
    let mut buf = vec![0; max_request_size + 1];
    loop {
        let n = match client_reader.read(&mut buf).await {
            Ok(n) if n == 0 => break,
            Ok(n) if n > max_request_size => {
                // 413 Request Entity Too Large
                let _ = client_writer
                    .write_all(b"HTTP/1.1 413 Request Entity Too Large\r\n\r\n")
                    .await;
                break;
            }
            Ok(n) => n,
            Err(e) => {
                eprintln!("failed to read from socket; err = {:?}", e);
                break;
            }
        };

        if n == 0 {
            break;
        }

        let (mut server_reader, mut server_writer) = match server_stream {
            Some(ref mut s) => s.split(),
            None => {
                let new_s = match TcpStream::connect(remote_server).await {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("Error connecting to {} -> {}", remote_server, e);
                        let http_response = format!(
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\n\r\n{}",
                            msg.len(),
                            msg
                        );
                        client_writer.write_all(http_response.as_bytes()).await?;
                        return Ok(());
                    }
                };
                server_stream = Some(new_s);
                server_stream.as_mut().unwrap().split()
            }
        };

        // println!("read {} bytes", n);
        server_writer.write_all(&buf[0..n]).await?;
        let n = server_reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        client_writer.write_all(&buf[0..n]).await?;
    }

    Ok(())
}
