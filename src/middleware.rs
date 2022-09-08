pub mod cache;

use crate::entity_meta::EntityMeta;
use anyhow::Result;
use async_trait::async_trait;
use std::{future::Future, pin::Pin, sync::Arc};

pub type DbRet<T> = Pin<Box<dyn Future<Output = Result<T>> + Send>>;
pub type DbNext<T> = Box<dyn FnOnce() -> DbRet<T> + Send + Sync>;

#[async_trait]
pub trait EventListener {
	async fn aggregate(
		self: Arc<Self>,
		operation: String,
		entity_meta: &'static EntityMeta,
		next: DbNext<i64>,
	) -> Result<i64>;
}

// pub struct DbMiddleware {
// 	pub count: Box<dyn Fn(&EntityMeta, &DbNext<T>) -> DbRet<T>>,
// }
