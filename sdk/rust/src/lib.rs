use async_trait::async_trait;
pub use nylon_abi;
pub use nylon_ring::{
    NrBytes, NrHostExt, NrHostVTable, NrKV, NrStatus, NrStr, NrVec, define_plugin,
};
pub use nylon_ring_host::NylonRingHost;
use std::ffi::c_void;

// pub crate
pub use once_cell;
pub use tokio;

pub enum NylonPluginStatus {
    Next,
    End,
}

#[async_trait]
pub trait NylonPlugin {
    async fn request_filter(&self) -> NylonPluginStatus {
        NylonPluginStatus::Next
    }
}

#[macro_export]
macro_rules! nylon_plugin {
    (
        initialize: $init_fn:path,
        shutdown: $shutdown_fn:path,
        plugins: {
            $($plugin_name:literal => $nylon_plugin_trait:path),* $(,)?
        }
    ) => {
        static mut NYLON_PLUGIN_HOST_CTX: *mut c_void = std::ptr::null_mut();
        static mut NYLON_PLUGIN_HOST_VTABLE: *const NrHostVTable = std::ptr::null();
        static NYLON_PLUGIN_TOKIO_RT: std::sync::OnceLock<$crate::tokio::runtime::Runtime> =
            std::sync::OnceLock::new();

        thread_local! {
            static NYLON_PLUGIN_ASYNC_QUEUE: $crate::once_cell::sync::OnceCell<$crate::tokio::sync::mpsc::UnboundedSender<(u64, NrBytes)>> = const { $crate::once_cell::sync::OnceCell::new() };
        }

        #[inline(always)]
        fn __nylon_get_runtime() -> &'static $crate::tokio::runtime::Runtime {
            NYLON_PLUGIN_TOKIO_RT.get_or_init(|| {
                let concurrency = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(8);
                $crate::tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(concurrency)
                    .enable_all()
                    .build()
                    .expect("Failed to create Tokio runtime")
            })
        }

        pub fn __nylon_async_worker_benchmark() {
            let (tx, mut rx) = $crate::tokio::sync::mpsc::unbounded_channel::<(u64, NrBytes)>();
            NYLON_PLUGIN_ASYNC_QUEUE.with(|cell| {
                cell.set(tx).ok();
            });

            __nylon_get_runtime().spawn(async move {
                while let Some((sid, payload)) = rx.recv().await {
                    let nr_vec = NrVec::from_nr_bytes(payload);
                    __nylon_send_result(sid, NrStatus::Ok, nr_vec);
                }
            });
        }

        #[inline(always)]
        pub fn __nylon_send_result(sid: u64, status: NrStatus, data: NrVec<u8>) {
            unsafe {
                let f = (*NYLON_PLUGIN_HOST_VTABLE).send_result;
                f(NYLON_PLUGIN_HOST_CTX, sid, status, data);
            };
        }

        #[inline(always)]
        fn plugin_init(host_ctx: *mut c_void, host_vtable: *const NrHostVTable) -> NrStatus {
            unsafe {
                NYLON_PLUGIN_HOST_CTX = host_ctx;
                NYLON_PLUGIN_HOST_VTABLE = host_vtable;
            }

            $crate::tokio::spawn(async move {
                $init_fn().await;
            });

            NrStatus::Ok
        }

        #[inline(always)]
        fn plugin_shutdown() {
            $crate::tokio::spawn(async move {
                $shutdown_fn().await;
            });
        }

        define_plugin! {
            init: plugin_init,
            shutdown: plugin_shutdown,
            entries: {}
        }
    };
}

async fn pp_init() {
    println!("pp_init");
}

async fn pp_shutdown() {
    println!("pp_shutdown");
}

pub struct TestPlugin;

#[async_trait]
impl NylonPlugin for TestPlugin {
    async fn request_filter(&self) -> NylonPluginStatus {
        NylonPluginStatus::Next
    }
}

// test
nylon_plugin! {
    initialize: pp_init,
    shutdown: pp_shutdown,
    plugins: {
        "test" => TestPlugin,
    }
}
