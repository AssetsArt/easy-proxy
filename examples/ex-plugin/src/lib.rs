use async_trait::async_trait;
use nylon_sdk::{NylonPlugin, NylonPluginStatus, nylon_plugin};

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
