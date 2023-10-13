use common::utoipa::{self, Modify, OpenApi};

// internal
use crate::resource;

// const
pub const BASE_PATH: &str = "/api/setup";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Setup API",
        version = "1.0.0",
        description = "An API to manage setup"
    ),
    paths(
        resource::installing,
        resource::is_installing
    ),
    modifiers(&ServerBase),
    components(
        schemas(
            resource::InstallingResponse,
            resource::InstallingBody,
            resource::IsInstallingResponse,
            resource::IsInstallingResponseData
        )
    )
)]
pub struct ApiDoc;

struct ServerBase;
impl Modify for ServerBase {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let paths = openapi.paths.paths.clone();
        openapi.paths.paths.clear();
        for (path, item) in paths.iter() {
            let path = format!("{}{}", BASE_PATH, path);
            openapi.paths.paths.insert(path, item.clone());
        }
    }
}
