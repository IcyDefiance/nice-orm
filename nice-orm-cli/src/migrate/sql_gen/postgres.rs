use std::collections::HashMap;

use super::SqlGen;
use anyhow::Result;
use async_trait::async_trait;
use itertools::Itertools;
use nice_orm::entity_meta::{Entities, EntityMeta, FieldMeta, FieldType};
use sqlx::{migrate::Migrator, postgres::PgPoolOptions, query_as, FromRow, PgPool, Pool, Postgres};

pub struct PostgresSqlGen {
	entities: Entities,
	pool: PgPool,
}
impl PostgresSqlGen {
	pub async fn new(entities: Entities, uri: &str) -> Result<Self> {
		Ok(Self { entities, pool: PgPoolOptions::new().connect(&uri).await? })
	}

	fn create_table(&self, entity: &EntityMeta) -> String {
		let mut lines = entity
			.fields
			.values()
			.map(|field| {
				let field_type = self.entity_type_to_column_type(field.ty);
				let not_null = if field.optional { "NULL" } else { "NOT NULL" };
				format!("\n\t\"{}\" {} {}", field.name, field_type, not_null)
			})
			.collect::<Vec<_>>();

		if entity.primary_key.len() > 0 {
			let primary_key = entity.primary_key.iter().map(|field| format!("\"{}\"", field)).collect::<Vec<_>>();
			lines.push(format!("\n\tPRIMARY KEY ({})", primary_key.join(", ")));
		}

		format!("CREATE TABLE \"{}\" ({}\n);\n", entity.table_name, lines.join(","))
	}

	fn drop_table(&self, table: &str) -> String {
		format!("DROP TABLE \"{}\";", table)
	}

	fn create_column(&self, table: &str, column: &FieldMeta) -> String {
		let field_type = self.entity_type_to_column_type(column.ty);
		let not_null = if column.optional { "NULL" } else { "NOT NULL" };
		format!("ALTER TABLE \"{}\" ADD COLUMN \"{}\" {} {};", table, column.name, field_type, not_null)
	}

	fn drop_column(&self, table: &str, column: &str) -> String {
		format!("ALTER TABLE \"{}\" DROP COLUMN \"{}\";", table, column)
	}

	fn update_column(&self, table: &str, column: &FieldMeta) -> String {
		let not_null = if column.optional { "NULL" } else { "NOT NULL" };
		format!(
			"ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {} {};",
			table,
			column.name,
			self.entity_type_to_column_type(column.ty),
			not_null
		)
	}

	fn entity_type_to_column_type(&self, ty: FieldType) -> &'static str {
		match ty {
			FieldType::I32 => "integer",
			FieldType::String => "varchar",
		}
	}
}
#[async_trait]
impl SqlGen for PostgresSqlGen {
	async fn gen_migration(&self) -> Result<(String, Option<String>)> {
		let old_schema = get_old_table_info(&self.pool).await?;

		let mut up = vec![];
		let mut down = Some(vec![]);

		// drop tables
		for table in old_schema.keys().filter(|k| !self.entities.contains_key(k)) {
			up.push(self.drop_table(table));
			down = None;
		}

		// create tables
		for table in self.entities.keys().filter(|&&k| !old_schema.contains_key(k)) {
			let entity = self.entities[table];
			up.push(self.create_table(entity));
			if let Some(down) = &mut down {
				down.push(self.drop_table(table));
			}
		}

		for (&table, &entity) in &*self.entities {
			if let Some(old_fields) = old_schema.get(table) {
				// drop columns
				for column in old_fields.keys().filter(|k| !entity.fields.contains_key(k)) {
					up.push(self.drop_column(table, column));
				}

				for &column in entity.fields.keys() {
					let column_meta = &entity.fields[column];
					if let Some(old_column) = old_fields.get(column) {
						// update columns
						if old_column.ty != self.entity_type_to_column_type(column_meta.ty) {
							up.push(self.update_column(table, &column_meta));
						}
					} else {
						// create columns
						let column = &column_meta;
						up.push(self.create_column(table, column));
						if let Some(down) = &mut down {
							down.push(self.drop_column(table, column.name));
						}
					}
				}
			}
		}

		Ok((up.join("\n"), down.map(|x| x.join("\n"))))
	}

	async fn run_migrations(&self, migrator: &Migrator) -> Result<()> {
		Ok(migrator.run(&self.pool).await?)
	}
}

async fn get_old_table_info(pool: &Pool<Postgres>) -> Result<HashMap<String, HashMap<String, PgField>>> {
	#[derive(FromRow)]
	struct FieldRow {
		table_name: String,
		column_name: String,
		data_type: String,
	}
	let fields_query = query_as::<_, FieldRow>(
		"SELECT table_name, column_name, data_type
		FROM information_schema.columns
		WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations';",
	);
	let fields = fields_query
		.fetch_all(pool)
		.await?
		.into_iter()
		.map(|x| (x.table_name, (x.column_name.clone(), PgField { name: x.column_name, ty: x.data_type })))
		.into_group_map();
	let mut fields: HashMap<String, HashMap<_, _>> =
		fields.into_iter().map(|(table, fields)| (table, fields.into_iter().collect())).collect();

	#[derive(FromRow)]
	struct TableRow {
		tablename: String,
	}
	let tables_query = query_as::<_, TableRow>(
		"SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname = 'public' AND tablename <> '_sqlx_migrations';",
	);
	for table in tables_query.fetch_all(pool).await? {
		fields.entry(table.tablename).or_insert(HashMap::new());
	}

	Ok(fields)
}

struct PgField {
	#[allow(unused)]
	name: String,
	ty: String,
}
