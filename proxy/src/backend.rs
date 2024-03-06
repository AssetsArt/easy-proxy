use config::proxy::BackendType;
use pingora::lb::{selection::BackendIter, Backend};

pub fn selected(service: &BackendType) -> Option<&'static Backend> {
    let backend: &Backend = match service {
        BackendType::RoundRobin(iter) => unsafe { iter.as_mut()?.next()? },
        BackendType::Weighted(iter) => unsafe { iter.as_mut()?.next()? },
        BackendType::Consistent(iter) => unsafe { iter.as_mut()?.next()? },
        BackendType::Random(iter) => unsafe { iter.as_mut()?.next()? },
    };
    Some(backend)
}
