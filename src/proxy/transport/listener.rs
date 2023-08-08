// Credit : https://github.dev/linkerd/linkerd2-proxy
/*
## License

linkerd2-proxy is copyright 2018 the linkerd2-proxy authors. All rights reserved.

Licensed under the Apache License, Version 2.0 (the "License"); you may not use
these files except in compliance with the License. You may obtain a copy of the
License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied. See the License for the
specific language governing permissions and limitations under the License.
*/
use crate::proxy::io;
use futures::prelude::*;
use socket2::TcpKeepalive;
use std::time::Duration;
use std::{fmt, net::SocketAddr, pin::Pin};
use tokio::net::TcpStream;
use tokio_stream::wrappers::TcpListenerStream;

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Wraps an address type to indicate it describes an address describing this
/// process.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Local<T>(pub T);

/// The address of a server.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ServerAddr(pub SocketAddr);

/// Wraps an address type to indicate it describes another process.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Remote<T>(pub T);

/// The address of a client.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClientAddr(pub SocketAddr);

/// Binds a listener, producing a stream of incoming connections.
///
/// Typically, this represents binding a TCP socket. However, it may also be an
/// stream of in-memory mock connections, for testing purposes.
pub trait Bind {
    type Io: io::AsyncRead
        + io::AsyncWrite
        + io::Peek
        + io::PeerAddr
        + fmt::Debug
        + Unpin
        + Send
        + Sync
        + 'static;
    type Addrs: Clone + Send + Sync + 'static;
    type Incoming: Stream<Item = Result<(Self::Addrs, Self::Io)>> + Send + Sync + 'static;

    fn bind(self, addr: &SocketAddr, keepalive: Option<Duration>) -> Result<Bound<Self::Incoming>>;
}

pub type Bound<I> = (Local<ServerAddr>, I);

#[derive(Clone, Debug)]
pub struct Addrs {
    pub server: Local<ServerAddr>,
    pub client: Remote<ClientAddr>,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct BindTcp(());

impl Bind for BindTcp {
    type Addrs = Addrs;
    type Incoming = Pin<Box<dyn Stream<Item = Result<(Self::Addrs, Self::Io)>> + Send + Sync>>;
    type Io = TcpStream;

    fn bind(self, addr: &SocketAddr, keepalive: Option<Duration>) -> Result<Bound<Self::Incoming>> {
        let listen = {
            let l = std::net::TcpListener::bind(addr)?;
            // Ensure that O_NONBLOCK is set on the socket before using it with Tokio.
            l.set_nonblocking(true)?;
            tokio::net::TcpListener::from_std(l).expect("listener must be valid")
        };
        let server = Local(ServerAddr(listen.local_addr()?));
        let accept = TcpListenerStream::new(listen).map(move |res| {
            let tcp = res.map_err(io::Error::from)?;
            set_nodelay_or_warn(&tcp);
            let tcp = set_keepalive_or_warn(tcp, keepalive).map_err(io::Error::from)?;
            let client = Remote(ClientAddr(tcp.peer_addr().map_err(io::Error::from)?));
            Ok((Addrs { server, client }, tcp))
        });

        Ok((server, Box::pin(accept)))
    }
}

fn set_keepalive_or_warn(
    tcp: TcpStream,
    keepalive_duration: Option<Duration>,
) -> io::Result<TcpStream> {
    let sock = {
        let stream = tokio::net::TcpStream::into_std(tcp)?;
        socket2::Socket::from(stream)
    };
    let ka = keepalive_duration
        .into_iter()
        .fold(TcpKeepalive::new(), |k, t| k.with_time(t));
    if let Err(e) = sock.set_tcp_keepalive(&ka) {
        tracing::warn!("failed to set keepalive: {}", e);
    }
    let stream: std::net::TcpStream = socket2::Socket::into(sock);
    tokio::net::TcpStream::from_std(stream)
}

fn set_nodelay_or_warn(socket: &TcpStream) {
    if let Err(e) = socket.set_nodelay(true) {
        tracing::warn!("failed to set nodelay: {}", e);
    }
}
