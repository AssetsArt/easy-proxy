use std::sync::OnceLock;
use proxy_common::{
    anyhow,
    bytes::Bytes,
    http_body_util::combinators::BoxBody,
    hyper::{
        self,
        client::conn::http1::{Builder, SendRequest},
    },
    hyper_util::rt::TokioIo,
    tokio::{
        self,
        net::TcpStream,
        sync::{Mutex, MutexGuard},
    },
};

fn random_id() -> u64 {
    let now = std::time::SystemTime::now();
    let duration = match now.duration_since(std::time::UNIX_EPOCH) {
        Ok(n) => n,
        Err(_) => {
            let random = rand::random::<u64>();
            return random;
        }
    };
    duration.as_secs() * 1_000_000_000 + duration.subsec_nanos() as u64
}

pub struct ManageConnection;
type SendRequestPool = Vec<(u64, Mutex<SendRequest<BoxBody<Bytes, hyper::Error>>>)>;
type SendRequestPoolId = Vec<(String, u64)>;

static mut CONNECTIONS: OnceLock<SendRequestPool> = OnceLock::new();
static mut CONNECTIONS_IDS: OnceLock<SendRequestPoolId> = OnceLock::new();

pub fn init() {
    unsafe {
        CONNECTIONS.get_or_init(|| Vec::new());
        CONNECTIONS_IDS.get_or_init(|| Vec::new());
    }
}

impl ManageConnection {
    pub async fn get(
        addr: String,
    ) -> Result<MutexGuard<'static, SendRequest<BoxBody<Bytes, hyper::Error>>>, anyhow::Error> {
        let conf = config::get_config();
        let conn_ids = unsafe {
            match CONNECTIONS_IDS.get_mut() {
                Some(c) => c,
                None => return Err(anyhow::anyhow!("[pool:45] CONNECTIONS_IDS"))
            }
        };
        let conn = unsafe { 
            match CONNECTIONS.get() {
                Some(c) => c,
                None => return Err(anyhow::anyhow!("[pool:52] CONNECTIONS"))
            }
        };

        if conn_ids.is_empty() {
            let id = random_id();
            conn_ids.push((addr.clone(), id));
            ManageConnection::new_connection(&mut (addr.clone(), id)).await;
        } else if conn_ids.iter().filter(|(a, _)| a == &addr).count()
            < conf.proxy.max_open_connections.into()
        {
            let id = random_id();
            conn_ids.push((addr.clone(), id));
            ManageConnection::new_connection(&mut (addr.clone(), id)).await;
        }
        let random = rand::random::<usize>() % conn_ids.iter().filter(|(a, _)| a == &addr).count();
        let state = conn_ids
            .iter()
            .filter(|(a, _)| a == &addr)
            .nth(random)
            .unwrap()
            .clone();
        let id = state.1;
        for (i_id, sender) in conn.iter() {
            if i_id == &id {
                let sender = sender.lock().await;
                return Ok(sender);
            }
        }
        Err(anyhow::anyhow!("[pool:81]"))
    }

    async fn new_connection(row: &mut (String, u64)) {
        let (addr, id) = row;
        let stream = TcpStream::connect(addr.clone()).await.unwrap();
        let io = TokioIo::new(stream);
        let (sender, conn) = Builder::new()
            .preserve_header_case(true)
            .title_case_headers(true)
            .handshake(io)
            .await
            .unwrap();

        let conns = unsafe { 
            match CONNECTIONS.get_mut() {
                Some(c) => c,
                None => return
            }
         };
        conns.push((*id, Mutex::new(sender)));

        let id = *id;
        let addr = addr.clone();
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                println!("Connection failed: {:?}", err);
                // remove
            }
            // remove
            let mm_id = unsafe { CONNECTIONS_IDS.get_mut().unwrap() };
            mm_id.retain(|(a, i)| a != &addr && i != &id);
            println!("Disconnect: {}", id);
        });
    }
}
