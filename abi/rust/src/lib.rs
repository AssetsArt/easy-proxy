use nylon_ring::{NrKV, NrStr, NrVec};
use pingora::proxy::Session;

#[derive(Debug)]
#[repr(C)]
pub struct NylonContext {
    pub client_ip: NrStr,
    pub path: NrStr,
    pub query: NrStr,
    pub method: NrStr,
    pub headers: NrVec<NrKV>,
}

impl Default for NylonContext {
    fn default() -> Self {
        Self {
            client_ip: NrStr::new("127.0.0.1"),
            path: NrStr::default(),
            query: NrStr::default(),
            method: NrStr::default(),
            headers: NrVec::default(),
        }
    }
}

impl AsRef<[u8]> for NylonContext {
    fn as_ref(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                self as *const Self as *const u8,
                std::mem::size_of::<Self>(),
            )
        }
    }
}

impl<'a> From<&'a [u8]> for &'a NylonContext {
    #[inline]
    fn from(value: &'a [u8]) -> Self {
        unsafe { &*(value.as_ptr() as *const NylonContext) }
    }
}

impl<'a> From<&'a Vec<u8>> for &'a NylonContext {
    fn from(value: &'a Vec<u8>) -> Self {
        unsafe { &*(value.as_ptr() as *const NylonContext) }
    }
}

impl NylonContext {
    #[inline]
    pub fn from_vec(vec: &Vec<u8>) -> &Self {
        <&NylonContext>::from(vec.as_slice())
    }

    #[inline]
    pub fn from_slice(slice: &[u8]) -> &Self {
        <&NylonContext>::from(slice)
    }
}

impl NylonContext {
    #[inline]
    pub fn parsed(&mut self, session: &Session) -> &Self {
        // ---------------------------
        // Reset fields for reuse
        // ---------------------------
        self.path.clear();
        self.query.clear();
        self.method.clear();
        self.headers.clear();

        // ---------------------------
        // Client IP
        // ---------------------------
        if let Some(inet) = session.client_addr().and_then(|a| a.as_inet()) {
            // ยังต้อง to_string() อยู่ เพราะ NrStr::new ต้องการ &str
            // แต่เราทำให้ NrStr reuse buffer เดิมได้ (ถ้า implement แบบ String-like)
            let ip_str = inet.ip().to_string();
            self.client_ip.clear();
            self.client_ip.push_str(&ip_str);
        } else {
            self.client_ip.clear();
            self.client_ip.push_str("127.0.0.1");
        }

        // ---------------------------
        // Headers + Path + Method
        // ---------------------------
        let (headers, uri, method) = if let Some(http2) = session.as_http2() {
            let req_header = http2.req_header();
            (&req_header.headers, &req_header.uri, &req_header.method)
        } else {
            let req_header = session.req_header();
            (&req_header.headers, &req_header.uri, &req_header.method)
        };

        // Path
        self.path.push_str(uri.path());

        // Query
        if let Some(q) = uri.query() {
            self.query.push_str(q);
        }

        // Method
        self.method.push_str(method.as_str());

        // ---------------------------
        // Headers (Vec instead of HashMap for speed)
        // ---------------------------
        // ensure capacity ใช้ len ปัจจุบัน ไม่โตเกินจำเป็น
        let needed = headers.len();
        if self.headers.capacity() < needed {
            self.headers.reserve(needed - self.headers.capacity());
        }

        for (k, v) in headers.iter() {
            let key = NrStr::new(k.as_str());
            if let Ok(v_str) = v.to_str() {
                let val = NrStr::new(v_str);
                self.headers.push(NrKV::from_nr_str(key, val));
            }
        }

        self
    }
}
