use crate::{entity_meta::EntityMeta, storage::Storage};
use anyhow::Result;

pub struct EntityManager<S> {
	storage: S,
}
impl<S: Storage> EntityManager<S> {
	pub fn new(storage: S) -> Self {
		Self { storage }
	}

	pub async fn create_table(&self, entity_meta: &EntityMeta) -> Result<()> {
		self.storage.create_table(entity_meta).await
	}
}
