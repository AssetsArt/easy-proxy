use common::{
    axum::{Router, Server},
    tokio,
    utoipa::OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() {
    let conf = config::get_config();

    // init database
    let _ = database::init().await;

    // address to listen on
    let addr = conf.runtime.api;
    let routes = Router::new()
        .nest(
            "/api",
            Router::new()
                .merge(api_auth::routes::Routes::new().get_routes())
                .merge(api_setup::routes::Routes::new().get_routes()),
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
        ]));

    println!("\n🚀 Listening on http://{}\n", addr);
    if let Err(e) = Server::bind(&addr.parse().unwrap())
        .serve(routes.into_make_service())
        .await
    {
        eprintln!("Server error: {}", e);
    }
}
