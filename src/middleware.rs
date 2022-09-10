pub mod cache;

use crate::{entity_meta::EntityMeta, Entity};
use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;

pub type AggregateNext<'a> =
	Box<dyn FnOnce(&'static str, &'static EntityMeta) -> BoxFuture<'a, Result<i64>> + Send + Sync + 'a>;
pub type FlushNext<'a> = Box<dyn FnOnce(&'a mut dyn Entity) -> BoxFuture<'a, Result<()>> + Send + Sync + 'a>;

#[async_trait]
pub trait EventListener {
	async fn aggregate(
		&self,
		operation: &'static str,
		entity_meta: &'static EntityMeta,
		next: AggregateNext<'async_trait>,
	) -> Result<i64>;

	async fn flush(&self, entity: &mut dyn Entity, next: FlushNext<'async_trait>) -> Result<()>;
}
