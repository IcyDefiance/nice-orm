pub mod file;

use crate::entity_meta::EntityMeta;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Storage {
	async fn create_table(&self, _entity_meta: &EntityMeta) -> Result<()>;
}
