#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub mod constants;
pub mod loaders;
mod native;
pub mod plugin_manager;
pub mod stream;
pub mod types;

use crate::{
    plugin_manager::PluginManager,
    types::{BuiltinPlugin, MiddlewareContext},
};
use bytes::Bytes;
use nylon_error::NylonError;
use nylon_types::context::NylonContext;
use nylon_types::plugins::PluginPhase;
use pingora::proxy::{ProxyHttp, Session};

pub async fn run_middleware<T>(
    _proxy: &T,
    _phase: &PluginPhase,
    middleware_context: &MiddlewareContext,
    ctx: &mut NylonContext,
    session: &mut Session,
    _response_body: &Option<Bytes>,
) -> Result<(bool, bool), NylonError>
where
    T: ProxyHttp + Send + Sync,
    <T as ProxyHttp>::CTX: Send + Sync + From<NylonContext>,
{
    let (middleware, payload, payload_ast) = (
        &middleware_context.middleware,
        &middleware_context.payload,
        &middleware_context.payload_ast,
    );
    // Allow builtin plugins to run without requiring an entry
    let (plugin_name_opt, _entry_opt) = (&middleware.plugin, &middleware.entry);
    let Some(plugin_name) = plugin_name_opt else {
        return Ok((false, false));
    };
    match PluginManager::try_builtin(plugin_name.as_str()) {
        Some(BuiltinPlugin::RequestHeaderModifier) => {
            native::header_modifier::request(ctx, session, payload, payload_ast)?;
            Ok((false, false))
        }
        Some(BuiltinPlugin::ResponseHeaderModifier) => {
            native::header_modifier::response(ctx, session, payload, payload_ast)?;
            Ok((false, false))
        }
        _ => {
            // For non-builtin plugins, require entry
            unimplemented!()
        }
    }
}
