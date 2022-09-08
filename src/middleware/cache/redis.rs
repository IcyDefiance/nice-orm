use crate::{
	entity_meta::EntityMeta,
	middleware::{DbNext, EventListener},
};
use anyhow::Result;
use async_trait::async_trait;
use futures::lock::Mutex;
use redis::{aio::Connection, AsyncCommands, Client};
use std::{collections::VecDeque, env, sync::Arc};

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
		self: Arc<Self>,
		operation: String,
		entity_meta: &'static EntityMeta,
		next: DbNext<i64>,
	) -> Result<i64> {
		let key = format!("{}:{}:{}", self.prefix, entity_meta.table_name, operation);
		let mut connection = self.get_connection().await?;
		let mut count: Option<i64> = connection.get(&key).await?;
		if count.is_none() {
			count = Some(next().await?);
			connection.set(&key, count).await?;
		}
		self.return_connection(connection).await;
		Ok(count.unwrap())
	}
}
