use crate::{
	entity_meta::EntityMeta,
	middleware::{AggregateNext, EventListener, FlushNext},
	query::{Predicate, SqlBuilder},
	Entity,
};
use anyhow::Result;
use async_trait::async_trait;
use deadpool_redis::{Config, Connection, Pool, Runtime};
use flate2::{write::ZlibEncoder, Compression};
use redis::{AsyncCommands, AsyncIter, Script};
use sqlx::{Postgres, Transaction};
use std::{fmt::Write as FmtWrite, io::Write, sync::Arc};

pub struct CacheRedis {
	pool: Pool,
	prefix: String,
}
impl CacheRedis {
	pub async fn new(prefix: String) -> Result<Self> {
		let cfg = Config::from_url("redis://127.0.0.1/");
		let pool = cfg.create_pool(Some(Runtime::Tokio1)).unwrap();
		Ok(Self { pool, prefix })
	}

	async fn get_connection(&self) -> Result<Connection> {
		Ok(self.pool.get().await?)
	}
}
#[async_trait]
impl EventListener for CacheRedis {
	async fn aggregate(
		self: Arc<Self>,
		operation: &'static str,
		entity_meta: &'static EntityMeta,
		filter: Option<&'async_trait Box<dyn Predicate + Send + Sync + 'async_trait>>,
		next: AggregateNext,
	) -> Result<i64> {
		let mut key = format!("{}:{}:{}", self.prefix, entity_meta.table_name, operation);
		if let Some(filter) = filter {
			// TODO: optimize by writing directly to compressor
			let mut sql = SqlBuilder::new();
			filter.push_to(&mut sql);
			let sql = sql.to_string();
			// TODO: maybe optimize by using a faster compression algorithm?
			let mut enc = ZlibEncoder::new(Vec::new(), Compression::fast());
			write!(enc, "{}", sql).unwrap();
			write!(key, ":{}", base64::encode(enc.finish().unwrap())).unwrap();
		}

		let mut redis = self.get_connection().await?;
		let mut count: Option<i64> = redis.get(&key).await?;
		if count.is_none() {
			count = Some(next(operation, entity_meta, filter).await?);
			redis.set(&key, count).await?;
		}

		Ok(count.unwrap())
	}

	async fn flush(
		self: Arc<Self>,
		transaction: &mut Transaction<'_, Postgres>,
		entity: &mut dyn Entity,
		next: FlushNext,
	) -> Result<()> {
		next(transaction, entity).await?;

		let key = format!("{}:{}:{}", self.prefix, entity.meta().table_name, "count");
		// TODO: save and reuse script
		let script = Script::new(
			"if redis.call('exists', ARGV[1]) then
				return redis.call('incr', ARGV[1])
			end",
		);
		let mut redis = self.get_connection().await?;
		script.arg(&key).invoke_async(&mut redis).await?;

		let mut iter: AsyncIter<Vec<String>> = redis.scan_match(&format!("{}:*", key)).await?;
		while let Some(keys) = iter.next_item().await {
			let mut redis = self.get_connection().await?;
			let _: i8 = redis.unlink(&keys).await?;
		}

		Ok(())
	}
}
