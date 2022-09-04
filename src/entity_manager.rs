use crate::{entity_meta::FieldType, Entity, Key};
use anyhow::Result;
use sqlx::{pool::PoolConnection, postgres::PgPoolOptions, query, PgPool, Postgres, Row};
use std::{any::TypeId, collections::HashMap, sync::Arc};
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
	entities: HashMap<TypeId, HashMap<Box<dyn Key + Send + Sync>, Arc<RwLock<dyn Entity>>>>,
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

	pub async fn save_changes(&mut self) -> Result<()> {
		println!("saving {} entities", self.pending_entities.len());
		for entity in self.pending_entities.drain(..) {
			let (id, type_id) = {
				println!("locking entity");
				let mut entity = entity.write().await;
				let entity = &mut *entity;
				let fields = &entity.meta().fields;
				let sql = format!(
					"INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING {}",
					entity.meta().table_name,
					fields.keys().map(|field| format!("\"{}\"", field)).collect::<Vec<_>>().join(", "),
					fields.keys().enumerate().map(|(i, _)| format!("${}", i + 1)).collect::<Vec<_>>().join(", "),
					entity
						.meta()
						.primary_key
						.iter()
						.map(|field| format!("\"{}\"", field))
						.collect::<Vec<_>>()
						.join(", "),
				);
				let mut query = query(&sql);
				for field in fields.values() {
					let value = entity.field(field.name).unwrap();
					query = match field.ty {
						FieldType::I32 => query.bind(value.downcast_ref::<i32>().unwrap()),
						FieldType::String => query.bind(value.downcast_ref::<String>().unwrap()),
					};
				}
				println!("executing query");
				let result = query.fetch_one(&mut self.connection).await?;
				println!("inserted");
				for field in entity.meta().primary_key.iter() {
					let field_meta = &entity.meta().fields[field];
					let value = entity.field_mut(field_meta.name).unwrap();
					match field_meta.ty {
						FieldType::I32 => *value.downcast_mut::<i32>().unwrap() = result.get(field),
						FieldType::String => *value.downcast_mut::<String>().unwrap() = result.get(field),
					}
				}
				(entity.id().unwrap(), (*entity).type_id())
			};
			self.entities.entry(type_id).or_insert_with(HashMap::new).insert(id, entity);
		}
		Ok(())
	}
}
