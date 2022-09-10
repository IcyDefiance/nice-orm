use crate::{
	entity_meta::EntityMeta,
	middleware::{AggregateNext, EventListener, FlushNext},
	Entity,
};
use anyhow::Result;
use async_trait::async_trait;
use futures::lock::Mutex;
use redis::{aio::Connection, AsyncCommands, Client, Script};
use std::{collections::VecDeque, env};

pub struct CacheRedis {
	client: Client,
	connections: Mutex<VecDeque<Connection>>,
	prefix: String,
}
impl CacheRedis {
	pub async fn new(prefix: String) -> Result<Self> {
		let client = redis::Client::open(env::var("REDIS_URL")?)?;
		Ok(Self { client, connections: Mutex::default(), prefix })
	}

	async fn get_connection(&self) -> Result<Connection> {
		if let Some(connection) = self.connections.lock().await.pop_front() {
			Ok(connection)
		} else {
			Ok(self.client.get_tokio_connection().await?)
		}
	}

	async fn return_connection(&self, connection: Connection) {
		self.connections.lock().await.push_back(connection);
	}
}
#[async_trait]
impl EventListener for CacheRedis {
	async fn aggregate(
		&self,
		operation: &'static str,
		entity_meta: &'static EntityMeta,
		next: AggregateNext<'async_trait>,
	) -> Result<i64> {
		let key = format!("{}:{}:{}", self.prefix, entity_meta.table_name, operation);
		let mut connection = self.get_connection().await?;
		let mut count: Option<i64> = connection.get(&key).await?;
		if count.is_none() {
			println!("Cache miss: {}", key);
			count = Some(next(operation, entity_meta).await?);
			connection.set(&key, count).await?;
		} else {
			println!("Cache hit: {}", key);
		}
		self.return_connection(connection).await;
		Ok(count.unwrap())
	}

	async fn flush(&self, entity: &mut dyn Entity, next: FlushNext<'async_trait>) -> Result<()> {
		let table_name = entity.meta().table_name;

		next(entity).await?;

		let key = format!("{}:{}:{}", self.prefix, table_name, "count");
		let script = Script::new(
			r"if redis.call('exists', ARGV[1]) then
				return redis.call('incr', ARGV[1])
			end",
		);
		let mut connection = self.get_connection().await?;
		println!("Incrementing cache: {}", key);
		script.arg(key).invoke_async(&mut connection).await?;
		self.return_connection(connection).await;

		Ok(())
	}
}
