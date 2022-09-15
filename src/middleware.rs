pub mod cache;

use std::sync::Arc;

use crate::{entity_meta::EntityMeta, query::Predicate, Entity};
use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;
use sqlx::{Postgres, Transaction};

// params: operation, entity_meta, filter
pub type AggregateNext = Box<
	dyn for<'a> FnOnce(
			&'static str,
			&'static EntityMeta,
			Option<&'a Box<dyn Predicate + Send + Sync + 'a>>,
		) -> BoxFuture<'a, Result<i64>>
		+ Send
		+ Sync,
>;
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
		filter: Option<&'async_trait Box<dyn Predicate + Send + Sync + 'async_trait>>,
		next: AggregateNext,
	) -> Result<i64>;

	async fn flush(
		self: Arc<Self>,
		transaction: &mut Transaction<'_, Postgres>,
		entity: &mut dyn Entity,
		next: FlushNext,
	) -> Result<()>;
}
