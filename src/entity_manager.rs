use crate::{
	entity_meta::FieldType,
	middleware::{AggregateNext, EventListener, FlushNext},
	query::{Predicate, SqlBuilder},
	Entity, EntityExt, EntityField, Key,
};
use anyhow::Result;
use futures::FutureExt;
use sqlx::{postgres::PgPoolOptions, query, PgPool, Postgres, Row, Transaction};
use std::{any::TypeId, collections::HashMap, fmt::Write, marker::PhantomData, mem, sync::Arc};
use tokio::sync::RwLock;

pub struct DbContextPool {
	pool: Arc<PgPool>,
	middlewares: Arc<RwLock<Vec<Arc<dyn EventListener + Send + Sync>>>>,
}
impl DbContextPool {
	pub async fn new(uri: &str) -> Result<Self> {
		let pool = Arc::new(PgPoolOptions::new().connect(&uri).await?);
		Ok(Self { pool, middlewares: Arc::default() })
	}

	pub async fn get_db_context(&self) -> Result<DbContext> {
		Ok(DbContext::new(self.pool.clone(), self.middlewares.clone()))
	}

	pub async fn add_middleware(&mut self, middleware: impl EventListener + Send + Sync + 'static) {
		self.middlewares.write().await.push(Arc::new(middleware));
	}
}

/// Intended to be short-lived, such as for a single request.
pub struct DbContext {
	pool: Arc<PgPool>,
	entities: HashMap<TypeId, HashMap<Box<dyn Key + Send + Sync>, Arc<RwLock<dyn Entity>>>>,
	pending_entities: Vec<Arc<RwLock<dyn Entity>>>,
	middlewares: Arc<RwLock<Vec<Arc<dyn EventListener + Send + Sync>>>>,
}
impl DbContext {
	pub fn new(pool: Arc<PgPool>, middlewares: Arc<RwLock<Vec<Arc<dyn EventListener + Send + Sync>>>>) -> Self {
		Self { pool, entities: HashMap::new(), pending_entities: Vec::new(), middlewares }
	}

	pub fn add<T: Entity>(&mut self, entity: T) -> Arc<RwLock<T>> {
		let entity = Arc::new(RwLock::new(entity));
		self.pending_entities.push(entity.clone());
		entity
	}

	pub fn select<T: EntityExt>(&self) -> SelectBuilder<T> {
		SelectBuilder::new(self)
	}

	pub async fn save_changes(&mut self) -> Result<()> {
		let mut pending_entities = mem::take(&mut self.pending_entities);
		let mut transaction = self.pool.begin().await?;
		for entity in pending_entities.drain(..) {
			let (id, type_id) = {
				let mut entity = entity.write().await;
				let entity = &mut *entity;

				// build middleware chain
				let middlewares = self.middlewares.read().await;
				let mut next: FlushNext = Box::new(move |transaction, entity| {
					async move { Self::insert_entity(transaction, entity).await }.boxed()
				});
				for middleware in middlewares.iter().cloned() {
					next = Box::new(move |transaction, entity| middleware.flush(transaction, entity, next));
				}

				next(&mut transaction, entity).await?;

				(entity.id(), (*entity).type_id())
			};
			self.entities.entry(type_id).or_insert_with(HashMap::new).insert(id, entity);
		}
		transaction.commit().await?;
		Ok(())
	}

	async fn insert_entity(connection: &mut Transaction<'_, Postgres>, entity: &mut dyn Entity) -> Result<()> {
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
		let result = query.fetch_one(connection).await?;

		for field in fields.values() {
			let value = entity.field_mut(field.name).unwrap();
			match field.ty {
				FieldType::I32 => {
					*value.downcast_mut::<EntityField<i32>>().unwrap() = EntityField::Set(result.get(&field.name))
				},
				FieldType::String => {
					*value.downcast_mut::<EntityField<String>>().unwrap() = EntityField::Set(result.get(&field.name))
				},
			}
		}

		Ok(())
	}
}

pub struct SelectBuilder<'a, T> {
	db_context: &'a DbContext,
	filter: Option<Box<dyn Predicate + Send + Sync + 'a>>,
	phantom: PhantomData<T>,
}
impl<'a, T: EntityExt> SelectBuilder<'a, T> {
	pub fn new(db_context: &'a DbContext) -> Self {
		Self { db_context, filter: None, phantom: PhantomData }
	}

	pub fn filter(mut self, predicate: impl Predicate + Send + Sync + 'a) -> Self {
		self.filter = Some(Box::new(predicate));
		self
	}

	pub async fn count(self) -> Result<i64> {
		let middlewares = self.db_context.middlewares.read().await;
		let next = self.build_aggregate_middleware(middlewares.iter().cloned()).await;
		next("COUNT", T::META, self.filter.as_ref()).await
	}

	async fn build_aggregate_middleware(
		&self,
		middlewares: impl Iterator<Item = Arc<dyn EventListener + Send + Sync>>,
	) -> AggregateNext {
		let pool = self.db_context.pool.clone();
		let mut next: AggregateNext = Box::new(move |operation, entity_meta, filter| {
			async move {
				let mut sql = SqlBuilder::new();
				write!(sql, "SELECT {}(*) FROM \"{}\"", operation, entity_meta.table_name).unwrap();
				if let Some(filter) = filter {
					write!(sql, " WHERE ").unwrap();
					filter.push_to(&mut sql);
				}
				let mut query = sql.to_query();
				if let Some(filter) = filter {
					query = filter.bind_to(query);
				}
				Ok(query.fetch_one(&*pool).await?.get(0))
			}
			.boxed()
		});
		for middleware in middlewares {
			next = Box::new(move |operation, entity_meta, filter| {
				middleware.aggregate(operation, entity_meta, filter, next)
			});
		}
		next
	}
}
