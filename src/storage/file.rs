use super::Storage;
use crate::entity_meta::EntityMeta;
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs::File;

pub struct FileStorage {
	pub base_path: PathBuf,
}
impl FileStorage {
	pub fn new(base_path: impl Into<PathBuf>) -> Self {
		Self { base_path: base_path.into() }
	}
}
#[async_trait]
impl Storage for FileStorage {
	async fn create_table(&self, entity_meta: &EntityMeta) -> Result<()> {
		File::create(self.base_path.join(format!("{}.json", entity_meta.table_name))).await?;
		Ok(())
	}
}
