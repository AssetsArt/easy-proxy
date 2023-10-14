use proxy_common::{
    hyper::upgrade::Upgraded,
    tokio::{self, net::TcpStream},
};
use proxy_io::tokiort::TokioIo;

// Create a TCP connection to host:port, build a tunnel between the connection and
// the upgraded connection
pub async fn tunnel(upgraded: Upgraded, addr: String) -> std::io::Result<()> {
    // Connect to remote server
    let mut server = TcpStream::connect(addr).await?;
    let mut upgraded = TokioIo::new(upgraded);
    // Proxying data
    tokio::io::copy_bidirectional(&mut upgraded, &mut server).await?;
    Ok(())
}
