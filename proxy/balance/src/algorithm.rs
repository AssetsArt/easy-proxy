use database::models::{Destination, Service};
use proxy_common::{anyhow, async_trait};

#[async_trait::async_trait]
pub trait Algorithm {
    // find destination by algorithm
    async fn distination(svc: &Service) -> Result<Destination, anyhow::Error>;
    // clear algorithm state
    fn clear();
    // remove algorithm state for service
    fn remove(svc: &Service);
}
