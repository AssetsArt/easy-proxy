#[cfg(not(debug_assertions))]
use mimalloc::MiMalloc;

#[cfg(not(debug_assertions))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use common::{
    axum::{Router, Server},
    tokio, tracing, tracing_subscriber,
    utoipa::OpenApi,
};
use futures_util::future::join;
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    // initialize the logger
    tracing_subscriber::fmt::init();

    let conf = config::get_config();

    // init database
    let _ = database::init().await;
    database::reload_svc().await;

    // start the proxy server
    let prox_svc = async move {
        match proxy::listen::Listen::new().listen().await {
            Ok(_) => {
                tracing::info!("Proxy server stopped");
            }
            Err(e) => {
                tracing::error!("Error: {}", e);
            }
        }
    };

    // address to listen on
    let addr = &conf.runtime.addr;
    let routes = Router::new()
        .nest(
            "/api",
            Router::new()
                .merge(api_auth::routes::Routes::new().get_routes())
                .merge(api_setup::routes::Routes::new().get_routes())
                .merge(api_service::routes::Routes::new().get_routes()),
        )
        .merge(SwaggerUi::new("/apidoc").urls(vec![
            (
                "/apidoc/auth/openapi.json".into(),
                api_auth::api_doc::ApiDoc::openapi(),
            ),
            (
                "/apidoc/setup/openapi.json".into(),
                api_setup::api_doc::ApiDoc::openapi(),
            ),
            (
                "/apidoc/service/openapi.json".into(),
                api_service::api_doc::ApiDoc::openapi(),
            ),
        ]));

    let app_svc = async move {
        tracing::info!("🚀 APIs server listening on http://{}", addr);
        if let Err(e) = Server::bind(&addr.parse().unwrap())
            .serve(routes.into_make_service())
            .await
        {
            eprintln!("Server error: {}", e);
        }
    };
    join(app_svc, prox_svc).await;
}
