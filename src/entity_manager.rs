use crate::{Entity, Key};
use anyhow::Result;
use sqlx::{pool::PoolConnection, postgres::PgPoolOptions, PgPool, Postgres};
use std::{
	any::{Any, TypeId},
	collections::HashMap,
	sync::Arc,
};
use tokio::sync::RwLock;

pub struct DbContextPool {
	pool: PgPool,
}
impl DbContextPool {
	pub async fn new(uri: &str) -> Result<Self> {
		let pool = PgPoolOptions::new().connect(&uri).await?;
		Ok(Self { pool })
	}

	pub async fn get_db_context(&self) -> Result<DbContext> {
		let connection = self.pool.acquire().await?;
		Ok(DbContext::new(connection))
	}
}

/// Intended to be short-lived, such as for a single request.
pub struct DbContext {
	connection: PoolConnection<Postgres>,
	entities: HashMap<TypeId, HashMap<Box<dyn Key>, Arc<RwLock<dyn Entity>>>>,
	pending_entities: Vec<Arc<RwLock<dyn Entity>>>,
}
impl DbContext {
	pub fn new(connection: PoolConnection<Postgres>) -> Self {
		Self { connection, entities: HashMap::new(), pending_entities: Vec::new() }
	}

	pub fn add<T: Entity>(&mut self, entity: T) -> Arc<RwLock<T>> {
		let entity = Arc::new(RwLock::new(entity));
		self.pending_entities.push(entity.clone());
		entity
	}

	pub fn save_changes(&mut self) {
		for entity in self.pending_entities.drain(..) {
			let type_id = entity.type_id();
			let id = entity.id();
			let fields = entity.meta().fields;
			self.entities.entry(type_id).or_insert_with(HashMap::new).insert(id.unwrap(), entity);
		}
	}
}
