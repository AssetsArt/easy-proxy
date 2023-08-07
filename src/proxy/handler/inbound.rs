use crate::proxy::handler::remote_stream;
use crate::proxy::response::Response;
use crate::proxy::transport::listener::Addrs;
use std::error::Error;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

pub async fn inbound(
    mut client_stream: TcpStream,
    _addr: Addrs,
    http_version: http::Version,
) -> Result<(), Box<dyn Error>> {
    let remote_server: SocketAddr = "127.0.0.1:3000".to_string().parse()?;
    let (mut client_reader, mut client_writer) = client_stream.split();
    let mut server_stream: Option<TcpStream> = None;

    // 4 MiB
    let max_request_size = 4 * 1024 * 1024;
    let mut buf = vec![0; max_request_size + 1];

    while let Ok(n) = client_reader.read(&mut buf).await {
        if n == 0 {
            break;
        } else if n > max_request_size {
            // 413 Request Entity Too Large
            let _ = client_writer
                .write_all(
                    Response::builder(http_version)
                        .request_entity_too_arge()
                        .as_slice(),
                )
                .await;
            break;
        }

        match remote_stream(&mut server_stream, remote_server).await {
            Ok(s) => Some(s),
            Err(e) => {
                let msg = format!("Error connecting to {} -> {}", remote_server, e);
                client_writer
                    .write_all(
                        Response::builder(http_version)
                            .internal_server_error(msg)
                            .as_slice(),
                    )
                    .await?;
                return Ok(());
            }
        };

        let (mut server_reader, mut server_writer) = server_stream.as_mut().unwrap().split();
        server_writer.write_all(&buf[0..n]).await?;

        let n = server_reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }

        client_writer.write_all(&buf[0..n]).await?;
    }
    Ok(())
}
