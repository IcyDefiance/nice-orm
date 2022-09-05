use crate::{entity_meta::FieldType, Entity, EntityField, Key};
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
		for entity in self.pending_entities.drain(..) {
			let (id, type_id) = {
				let mut entity = entity.write().await;
				let entity = &mut *entity;

				let fields = &entity.meta().fields;

				let mut field_names = Vec::with_capacity(fields.len());
				let mut modified_field_names = Vec::with_capacity(fields.len());
				let mut modified_field_params = Vec::with_capacity(fields.len());
				for field in fields.values() {
					let value = entity.field(field.name).unwrap();
					let is_modified = match field.ty {
						FieldType::I32 => value.downcast_ref::<EntityField<i32>>().unwrap().is_modified(),
						FieldType::String => value.downcast_ref::<EntityField<String>>().unwrap().is_modified(),
					};
					field_names.push(format!("\"{}\"", field.name));
					if is_modified {
						modified_field_names.push(format!("\"{}\"", field.name));
						modified_field_params.push(format!("${}", modified_field_params.len() + 1));
					}
				}

				let sql = format!(
					"INSERT INTO \"{}\" ({}) VALUES ({}) RETURNING {}",
					entity.meta().table_name,
					modified_field_names.join(", "),
					modified_field_params.join(", "),
					field_names.join(", "),
				);

				let mut query = query(&sql);
				for field in fields.values() {
					let value = entity.field(field.name).unwrap();
					query = match field.ty {
						FieldType::I32 => query.bind(value.downcast_ref::<i32>().unwrap()),
						FieldType::String => query.bind(value.downcast_ref::<String>().unwrap()),
					};
				}
				let result = query.fetch_one(&mut self.connection).await?;

				for field in fields.values() {
					let value = entity.field_mut(field.name).unwrap();
					match field.ty {
						FieldType::I32 => {
							*value.downcast_mut::<EntityField<i32>>().unwrap() =
								EntityField::Set(result.get(&field.name))
						},
						FieldType::String => {
							*value.downcast_mut::<EntityField<String>>().unwrap() =
								EntityField::Set(result.get(&field.name))
						},
					}
				}

				(entity.id(), (*entity).type_id())
			};
			self.entities.entry(type_id).or_insert_with(HashMap::new).insert(id, entity);
		}
		Ok(())
	}
}
