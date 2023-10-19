use proxy_common::{
    anyhow,
    bytes::Bytes,
    http_body_util::combinators::BoxBody,
    hyper::{
        self,
        client::conn::http1::{Builder, SendRequest},
    },
    tokio::{self, net::TcpStream},
};
use proxy_io as io;
use std::collections::HashMap;
use std::sync::atomic::{AtomicPtr, Ordering};

fn get_datetime() -> u64 {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
}

pub struct ManageConnection;

#[derive(Default)]
pub struct Connections {
    pub senders: HashMap<String, HashMap<u64, SendRequest<BoxBody<Bytes, hyper::Error>>>>,
}

static CONNECTIONS: AtomicPtr<Connections> = AtomicPtr::new(std::ptr::null_mut());

pub fn get_connections() -> &'static mut Connections {
    unsafe {
        let ptr = CONNECTIONS.load(Ordering::Relaxed);
        if ptr.is_null() {
            let conn = Box::default();
            let ptr = Box::into_raw(conn);
            CONNECTIONS.store(ptr, Ordering::Relaxed);
        }
        &mut *CONNECTIONS.load(Ordering::Relaxed)
    }
}

impl ManageConnection {
    pub async fn pool(addr: String) -> Result<u64, anyhow::Error> {
        let conn = get_connections();
        let senders = &mut conn.senders;
        let sender_pool = match senders.get_mut(&addr) {
            Some(s) => s,
            None => {
                senders.insert(addr.clone(), HashMap::new());
                senders.get_mut(&addr).unwrap()
            }
        };
        if sender_pool.is_empty() {
            Self::new_connection(addr.clone(), sender_pool).await?;
        }

        let len = sender_pool.len();
        let conf = config::get_config();
        if len < conf.proxy.max_open_connections as usize {
            Self::new_connection(addr.clone(), sender_pool).await?;
        }
        // random select a connection
        let index = rand::random::<usize>() % len;
        let id = *sender_pool.keys().nth(index).unwrap();
        Ok(id)
    }

    async fn new_connection(
        addr: String,
        pool: &mut HashMap<u64, SendRequest<BoxBody<Bytes, hyper::Error>>>,
    ) -> Result<(), anyhow::Error> {
        let id = get_datetime();
        let stream = match TcpStream::connect(addr.clone()).await {
            Ok(s) => s,
            Err(e) => {
                return Err(anyhow::anyhow!("connect error: {}", e));
            }
        };
        let io = io::tokiort::TokioIo::new(stream);
        let (sender, conn) = Builder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake::<_, BoxBody<Bytes, hyper::Error>>(io)
            .await?;

        tokio::task::spawn(async move {
            if conn.await.is_ok() {
                let conn = get_connections();
                let senders = &mut conn.senders;
                if let Some(sender) = senders.get_mut(&addr) {
                    sender.remove(&id);
                }
                println!("Connection closed normally ip: {}, id: {}", addr, id);
            }
        });

        pool.insert(id, sender);

        Ok(())
    }
}
