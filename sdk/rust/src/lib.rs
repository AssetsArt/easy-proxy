use async_trait::async_trait;
pub use nylon_abi;
pub use nylon_ring::{
    NrBytes, NrHostExt, NrHostVTable, NrKV, NrStatus, NrStr, NrVec, define_plugin,
};
pub use nylon_ring_host::NylonRingHost;
use std::ffi::c_void;

// pub crate
pub use once_cell;
pub use paste;
pub use tokio;

pub enum NylonPluginStatus {
    Next,
    End,
}

#[async_trait]
pub trait NylonPlugin: Send + Sync + 'static {
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
            $($plugin_name:literal : $ty:ty),* $(,)?
        }
    ) => {$crate::paste::paste! {
        static NYLON_PLUGIN_HOST_CTX: std::sync::atomic::AtomicPtr<std::ffi::c_void> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
        static NYLON_PLUGIN_HOST_VTABLE: std::sync::atomic::AtomicPtr<NrHostVTable> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
        static NYLON_PLUGIN_TOKIO_RT: std::sync::OnceLock<$crate::tokio::runtime::Runtime> =
            std::sync::OnceLock::new();

        $(
            static [<NYLON_PLUGIN_HANDLE_ $plugin_name:upper>]: std::sync::OnceLock<&'static dyn $crate::NylonPlugin> = std::sync::OnceLock::new();
        )*

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

        #[inline(always)]
        pub fn __nylon_send_result(sid: u64, status: NrStatus, data: NrVec<u8>) {
            unsafe {
                let vtable = NYLON_PLUGIN_HOST_VTABLE.load(std::sync::atomic::Ordering::Relaxed) as *const NrHostVTable;
                let ctx = NYLON_PLUGIN_HOST_CTX.load(std::sync::atomic::Ordering::Relaxed);
                let f = (*vtable).send_result;
                f(ctx, sid, status, data);
            };
        }

        #[inline(always)]
        fn __nylon_plugin_init(host_ctx: *mut c_void, host_vtable: *const NrHostVTable) -> NrStatus {
            NYLON_PLUGIN_HOST_CTX.store(host_ctx, std::sync::atomic::Ordering::SeqCst);
            NYLON_PLUGIN_HOST_VTABLE.store(host_vtable as *mut _, std::sync::atomic::Ordering::SeqCst);

            $(
                let plugin_ref = Box::leak(Box::new(<$ty as Default>::default()));
                let _ = [<NYLON_PLUGIN_HANDLE_ $plugin_name:upper>].set(plugin_ref);
            )*

            // initialize async workers
            $([<__nylon_async_worker_ $plugin_name:lower>]();)*

            $crate::tokio::spawn(async move {
                $init_fn().await;
            });

            NrStatus::Ok
        }

        #[inline(always)]
        fn __nylon_plugin_shutdown() {
            $crate::tokio::spawn(async move {
                $shutdown_fn().await;
            });
        }

        thread_local! {
            $(
            static [<NYLON_PLUGIN_ASYNC_QUEUE_ $plugin_name:upper>]: $crate::once_cell::sync::OnceCell<$crate::tokio::sync::mpsc::UnboundedSender<(u64, NrBytes)>> = const { $crate::once_cell::sync::OnceCell::new() };
            )*
        }

        $(
        pub fn [<__nylon_async_worker_ $plugin_name:lower>]() {
            let (tx, mut rx) = $crate::tokio::sync::mpsc::unbounded_channel::<(u64, NrBytes)>();
            [<NYLON_PLUGIN_ASYNC_QUEUE_ $plugin_name:upper>].with(|cell| {
                cell.set(tx).ok();
            });

            __nylon_get_runtime().spawn(async move {
                while let Some((sid, payload)) = rx.recv().await {
                    let nr_vec = NrVec::from_nr_bytes(payload);

                    let plugin: &dyn $crate::NylonPlugin = *[<NYLON_PLUGIN_HANDLE_ $plugin_name:upper>].get().expect("Plugin initialized");
                    plugin.request_filter().await;

                    __nylon_send_result(sid, NrStatus::Ok, nr_vec);
                }
            });
        }
        )*

        $(
            #[inline(always)]
            fn [< __nylon_plugin_ $plugin_name:lower _wrapper >] (sid: u64, payload: $crate::NrBytes) -> $crate::NrStatus {
                println!("sid {}", sid);
                println!("payload {:?}", payload);
                [<NYLON_PLUGIN_ASYNC_QUEUE_ $plugin_name:upper>].with(|cell| {
                    if let Some(tx) = cell.get() {
                        let _ = tx.send((sid, payload));
                        return NrStatus::Ok;
                    }
                    NrStatus::Err
                })
            }
        )*

        define_plugin! {
            init: __nylon_plugin_init,
            shutdown: __nylon_plugin_shutdown,
            entries: {
                $($plugin_name => [<__nylon_plugin_ $plugin_name:lower _wrapper>]),*
            }
        }
    }}
}

async fn pp_init() {
    println!("pp_init");
}

async fn pp_shutdown() {
    println!("pp_shutdown");
}

#[derive(Default)]
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
        "test": TestPlugin,
    }
}
