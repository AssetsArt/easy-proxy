use common::utoipa::{self, ToSchema};
use serde::{Deserialize, Serialize};

use super::{Destination, ServiceBodyInput};

static PROTOCOL_SUPPORTED: [&str; 1] = ["http"];
static ALGORITHM_SUPPORTED: [&str; 1] = ["round-robin"];

#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
pub enum ValidateError {
    InvalidInput(String),
}

fn algorithm_validate(algorithm: &str) -> Result<bool, ValidateError> {
    if algorithm.is_empty() {
        return Err(ValidateError::InvalidInput(
            "Algorithm cannot be empty".into(),
        ));
    }

    if !ALGORITHM_SUPPORTED.contains(&algorithm) {
        return Err(ValidateError::InvalidInput(format!(
            "Algorithm not supported. Supported algorithms: {:?}",
            ALGORITHM_SUPPORTED
        )));
    }

    Ok(true)
}

fn destination_validate(destination: &Vec<Destination>) -> Result<bool, ValidateError> {
    if destination.is_empty() {
        return Err(ValidateError::InvalidInput(
            "Destination cannot be empty".into(),
        ));
    }

    for dest in destination.iter() {
        if dest.ip.is_empty() {
            return Err(ValidateError::InvalidInput(
                "Destination IP cannot be empty".into(),
            ));
        }

        if dest.port == 0 {
            return Err(ValidateError::InvalidInput(
                "Destination port cannot be empty".into(),
            ));
        }

        if dest.max_conn == 0 {
            return Err(ValidateError::InvalidInput(
                "Destination max connection cannot be empty".into(),
            ));
        }
    }

    Ok(true)
}

pub fn validate_add(input: &ServiceBodyInput) -> Result<bool, ValidateError> {
    if input.name.is_empty() {
        return Err(ValidateError::InvalidInput("Name cannot be empty".into()));
    }
    // name is unique allowed characters: a-z, 0-9, -, _
    // regex: ^[a-z0-9_-]+$
    let regex = common::regex::Regex::new(r"^[a-z0-9_-]+$").unwrap();
    if !regex.is_match(&input.name) {
        return Err(ValidateError::InvalidInput(
            "Name can only contain a-z, 0-9, -, _".into(),
        ));
    }

    destination_validate(&input.destination)?;
    algorithm_validate(input.algorithm.as_str())?;

    if input.protocol.is_empty() {
        return Err(ValidateError::InvalidInput(
            "Destination protocol cannot be empty".into(),
        ));
    }

    if !PROTOCOL_SUPPORTED.contains(&input.protocol.as_str()) {
        return Err(ValidateError::InvalidInput(format!(
            "Protocol not supported. Supported protocols: {:?}",
            PROTOCOL_SUPPORTED
        )));
    }

    if input.host.is_empty() {
        return Err(ValidateError::InvalidInput("Host cannot be empty".into()));
    }

    Ok(true)
}
