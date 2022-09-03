use std::collections::HashMap;

use super::SqlGen;
use async_trait::async_trait;
use nice_orm::entity_meta::{Entities, EntityMeta, FieldMeta, FieldType};

pub struct PostgresSqlGen {
	entities: Entities,
}
impl PostgresSqlGen {
	pub async fn new(entities: Entities, uri: &str) -> Result<Self> {
		Ok(Self { entities })
	}
}
#[async_trait]
impl SqlGen for PostgresSqlGen {}
