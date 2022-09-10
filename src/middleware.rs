pub mod cache;

use std::sync::Arc;

use crate::{entity_meta::EntityMeta, Entity};
use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;
use sqlx::{Postgres, Transaction};

pub type AggregateNext = Box<dyn FnOnce(&'static str, &'static EntityMeta) -> BoxFuture<Result<i64>> + Send + Sync>;
pub type FlushNext = Box<
	dyn for<'b> FnOnce(&'b mut Transaction<'_, Postgres>, &'b mut dyn Entity) -> BoxFuture<'b, Result<()>>
		+ Send
		+ Sync,
>;

#[async_trait]
pub trait EventListener {
	async fn aggregate(
		self: Arc<Self>,
		operation: &'static str,
		entity_meta: &'static EntityMeta,
		next: AggregateNext,
	) -> Result<i64>;

	async fn flush(
		self: Arc<Self>,
		transaction: &mut Transaction<'_, Postgres>,
		entity: &mut dyn Entity,
		next: FlushNext,
	) -> Result<()>;
}
