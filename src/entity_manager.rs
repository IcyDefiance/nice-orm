use crate::{
	entity_meta::FieldType,
	middleware::{DbNext, DbRet, EventListener},
	Entity, EntityExt, EntityField, Key,
};
use anyhow::Result;
use futures::FutureExt;
use sqlx::{pool::PoolConnection, postgres::PgPoolOptions, query, PgPool, Postgres, Row};
use std::{any::TypeId, collections::HashMap, marker::PhantomData, mem, sync::Arc};
use tokio::sync::RwLock;

pub struct DbContextPool {
	pool: Arc<PgPool>,
	middlewares: Vec<Arc<dyn EventListener + Send + Sync>>,
}
impl DbContextPool {
	pub async fn new(uri: &str) -> Result<Self> {
		let pool = Arc::new(PgPoolOptions::new().connect(&uri).await?);
		Ok(Self { pool, middlewares: vec![] })
	}

	pub async fn get_db_context(&self) -> Result<DbContext> {
		Ok(DbContext::new(self.pool.clone(), self.middlewares.clone()))
	}

	pub fn add_middleware(&mut self, middleware: impl EventListener + Send + Sync + 'static) {
		self.middlewares.push(Arc::new(middleware));
	}
}

/// Intended to be short-lived, such as for a single request.
pub struct DbContext {
	pool: Arc<PgPool>,
	entities: HashMap<TypeId, HashMap<Box<dyn Key + Send + Sync>, Arc<RwLock<dyn Entity>>>>,
	pending_entities: Vec<Arc<RwLock<dyn Entity>>>,
	middlewares: Vec<Arc<dyn EventListener + Send + Sync>>,
}
impl DbContext {
	pub fn new(pool: Arc<PgPool>, middlewares: Vec<Arc<dyn EventListener + Send + Sync>>) -> Self {
		Self { pool, entities: HashMap::new(), pending_entities: Vec::new(), middlewares }
	}

	pub fn add<T: Entity>(&mut self, entity: T) -> Arc<RwLock<T>> {
		let entity = Arc::new(RwLock::new(entity));
		self.pending_entities.push(entity.clone());
		entity
	}

	pub fn select<T: EntityExt>(&self) -> SelectQueryBuilder<T> {
		SelectQueryBuilder::new(self)
	}

	pub async fn save_changes(&mut self) -> Result<()> {
		let mut pending_entities = mem::take(&mut self.pending_entities);
		for entity in pending_entities.drain(..) {
			let (id, type_id) = {
				let mut entity = entity.write().await;
				let entity = &mut *entity;

				let fields = &entity.meta().fields;

				let mut field_names = Vec::with_capacity(fields.len());
				let mut modified_fields = Vec::with_capacity(fields.len());
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
						modified_fields.push(field);
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
				for field in modified_fields {
					let value = entity.field(field.name).unwrap();
					query = match field.ty {
						FieldType::I32 => query.bind(value.downcast_ref::<EntityField<i32>>().unwrap().get()),
						FieldType::String => query.bind(value.downcast_ref::<EntityField<String>>().unwrap().get()),
					};
				}
				let result = query.fetch_one(&mut self.get_connection().await?).await?;

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

	pub(crate) async fn get_connection(&self) -> Result<PoolConnection<Postgres>> {
		self.pool.acquire().await.map_err(Into::into)
	}
}

pub struct SelectQueryBuilder<'a, T> {
	db_context: &'a DbContext,
	phantom: PhantomData<T>,
}
impl<'a, T: EntityExt> SelectQueryBuilder<'a, T> {
	pub fn new(db_context: &'a DbContext) -> Self {
		Self { db_context, phantom: PhantomData }
	}

	pub async fn count(self) -> Result<i64> {
		let next = self.build_aggregate_middleware("COUNT", self.db_context.middlewares.clone().into_iter());
		next().await
	}

	fn build_aggregate_middleware(
		&self,
		operation: &str,
		middlewares: impl Iterator<Item = Arc<dyn EventListener + Send + Sync>>,
	) -> DbNext<i64> {
		let pool = self.db_context.pool.clone();
		let sql = format!("SELECT {}(*) FROM \"{}\";", operation, T::META.table_name);
		let mut next: Box<dyn FnOnce() -> DbRet<i64> + Send + Sync> = Box::new(move || -> DbRet<i64> {
			let sql = sql.clone();
			let connection = pool.acquire();
			async move { Ok(query(&sql).fetch_one(&mut connection.await?).await?.get(0)) }.boxed()
		});
		for middleware in middlewares {
			let operation = operation.to_string();
			next = Box::new(move || middleware.aggregate(operation.clone(), T::META, next));
		}
		next
	}
}
