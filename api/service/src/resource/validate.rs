use super::{AddServiceBody, AddServiceResponse, Destination};
use common::{axum::body::Body, http::StatusCode, serde_json::json};

static PROTOCOL_SUPPORTED: [&str; 1] = ["http"];
static ALGORITHM_SUPPORTED: [&str; 1] = ["round-robin"];

fn algorithm_validate(
    algorithm: &str,
    res: &mut AddServiceResponse,
) -> Option<common::http::Response<Body>> {
    if algorithm.is_empty() {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = "Algorithm cannot be empty".into();
        return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
    }

    if !ALGORITHM_SUPPORTED.contains(&algorithm) {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = format!(
            "Algorithm not supported. Supported algorithms: {:?}",
            ALGORITHM_SUPPORTED
        );
        return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
    }

    None
}

fn destination_validate(
    destination: &Vec<Destination>,
    res: &mut AddServiceResponse,
) -> Option<common::http::Response<Body>> {
    if destination.is_empty() {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = "Destination cannot be empty".into();
        return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
    }

    for dest in destination.iter() {
        if dest.ip.is_empty() {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = "Destination IP cannot be empty".into();
            return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
        }

        if dest.port == 0 {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = "Destination port cannot be empty".into();
            return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
        }

        if dest.protocol.is_empty() {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = "Destination protocol cannot be empty".into();
            return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
        }

        if !PROTOCOL_SUPPORTED.contains(&dest.protocol.as_str()) {
            res.status = StatusCode::BAD_REQUEST.into();
            res.message = format!(
                "Protocol not supported. Supported protocols: {:?}",
                PROTOCOL_SUPPORTED
            );
            return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
        }
    }
    None
}

pub fn validate_add(
    input: AddServiceBody,
    res: &mut AddServiceResponse,
) -> Option<common::http::Response<Body>> {
    if input.name.is_empty() {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = "Name cannot be empty".into();
        return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
    }
    // name is unique allowed characters: a-z, 0-9, -, _
    // regex: ^[a-z0-9_-]+$
    let regex = common::regex::Regex::new(r"^[a-z0-9_-]+$").unwrap();
    if !regex.is_match(&input.name) {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = "Name can only contain a-z, 0-9, -, _".into();
        return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
    }

    if let Some(res) = destination_validate(&input.destination, res) {
        return Some(res);
    }

    if let Some(res) = algorithm_validate(input.algorithm.as_str(), res) {
        return Some(res);
    }

    if input.host.is_empty() {
        res.status = StatusCode::BAD_REQUEST.into();
        res.message = "Host cannot be empty".into();
        return Some(common::response::json(json!(res), StatusCode::BAD_REQUEST));
    }

    None
}
