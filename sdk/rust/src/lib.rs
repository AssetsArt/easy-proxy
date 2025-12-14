pub use nylon_abi;
pub use nylon_ring::{
    NrBytes, NrHostExt, NrHostVTable, NrKV, NrStatus, NrStr, NrVec, define_plugin,
};
pub use nylon_ring_host::NylonRingHost;
use std::ffi::c_void;

static mut HOST_CTX: *mut c_void = std::ptr::null_mut();
static mut HOST_VTABLE: *const NrHostVTable = std::ptr::null();

#[inline(always)]
fn plugin_init(host_ctx: *mut c_void, host_vtable: *const NrHostVTable) -> NrStatus {
    unsafe {
        HOST_CTX = host_ctx;
        HOST_VTABLE = host_vtable;
    }
    NrStatus::Ok
}

#[inline(always)]
fn plugin_shutdown() {}

#[inline(always)]
pub fn send_result(sid: u64, status: NrStatus, data: NrVec<u8>) {
    unsafe {
        let f = (*HOST_VTABLE).send_result;
        f(HOST_CTX, sid, status, data);
    };
}

define_plugin! {
    init: plugin_init,
    shutdown: plugin_shutdown,
    entries: {}
}
