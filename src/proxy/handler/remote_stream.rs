use std::net::SocketAddr;
use tokio::net::TcpStream;

pub async fn remote_stream(
    stream: &mut Option<TcpStream>,
    remote: SocketAddr,
) -> Result<&mut Option<TcpStream>, String> {
    match stream {
        Some(_) => {}
        None => {
            match TcpStream::connect(remote).await {
                Ok(s) => *stream = Some(s),
                Err(e) => Err(e.to_string())?,
            };
        }
    };
    Ok(stream)
}
