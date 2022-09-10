pub mod cache;

use std::sync::Arc;

use crate::{entity_meta::EntityMeta, Entity};
use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;
use sqlx::{Postgres, Transaction};

pub type AggregateNext<'a> =
	Box<dyn FnOnce(&'static str, &'static EntityMeta) -> BoxFuture<'a, Result<i64>> + Send + Sync + 'a>;
pub type FlushNext<'a> = Box<
	dyn for<'b> FnOnce(&'b mut Transaction<'_, Postgres>, &'b mut dyn Entity) -> BoxFuture<'b, Result<()>>
		+ Send
		+ Sync
		+ 'a,
>;

#[async_trait]
pub trait EventListener {
	async fn aggregate(
		self: Arc<Self>,
		operation: &'static str,
		entity_meta: &'static EntityMeta,
		next: AggregateNext<'async_trait>,
	) -> Result<i64>;

	async fn flush(
		self: Arc<Self>,
		transaction: &mut Transaction<'_, Postgres>,
		entity: &mut dyn Entity,
		next: FlushNext<'async_trait>,
	) -> Result<()>;
}
