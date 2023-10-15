use proxy_common::{
    anyhow,
    bytes::Bytes,
    http_body_util::combinators::BoxBody,
    hyper::{
        self,
        client::conn::http1::{Builder, SendRequest},
    },
    tokio::{
        self,
        net::TcpStream,
        sync::Mutex,
    },
};
use proxy_io as io;
use std::collections::HashMap;

fn get_datetime() -> u64 {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
}

pub struct ManageConnection;

lazy_static::lazy_static! {
    pub static ref CONNECTION: Mutex<HashMap<String, HashMap<u64, SendRequest<BoxBody<Bytes, hyper::Error>>>>> =
        Mutex::new(HashMap::new());
}

impl ManageConnection {
    pub async fn pool(addr: String) -> Result<u64, anyhow::Error> {
        let mut connect = CONNECTION.lock().await;
        let sender_pool = match connect.get_mut(&addr) {
            Some(s) => s,
            None => {
                connect.insert(addr.clone(), HashMap::default());
                connect.get_mut(&addr).unwrap()
            }
        };
        if sender_pool.is_empty() {
            Self::new_connection(addr.clone(), sender_pool).await?;
        }
        let len = sender_pool.len();
        let conf = config::get_config();
        if len < conf.proxy.max_open_connections as usize {
            tokio::task::spawn(async move {
                let mut connect = CONNECTION.lock().await;
                let sender_pool = match connect.get_mut(&addr) {
                    Some(s) => s,
                    None => {
                        connect.insert(addr.clone(), HashMap::default());
                        connect.get_mut(&addr).unwrap()
                    }
                };
                Self::new_connection(addr.clone(), sender_pool).await.unwrap();
            });
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
        pool.insert(id, sender);
        tokio::task::spawn(async move {
            if conn.await.is_ok() {
                let mut connect = CONNECTION.lock().await;
                let sender_pool = match connect.get_mut(&addr) {
                    Some(s) => s,
                    None => {
                        connect.insert(addr.clone(), HashMap::default());
                        connect.get_mut(&addr).unwrap()
                    }
                };
                sender_pool.remove(&id);
                println!("Connection closed normally ip: {}, id: {}", addr, id);
            }
        });
        Ok(())
    }
}
