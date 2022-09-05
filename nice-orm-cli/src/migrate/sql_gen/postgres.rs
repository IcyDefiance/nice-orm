use std::collections::HashMap;

use super::SqlGen;
use anyhow::Result;
use async_trait::async_trait;
use itertools::Itertools;
use nice_orm::entity_meta::{Entities, EntityMeta, FieldMeta, FieldType, IdentityGeneration};
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
				let field_type = Self::entity_type_to_column_type(field.ty);
				let column_constraints = Self::make_column_constraints(field);
				format!("\n\t\"{}\" {} {}", field.name, field_type, column_constraints)
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

	fn create_column(&self, table: &str, field: &FieldMeta) -> String {
		let field_type = Self::entity_type_to_column_type(field.ty);
		let column_constraints = Self::make_column_constraints(field);
		format!("ALTER TABLE \"{}\" ADD COLUMN \"{}\" {} {};", table, field.name, field_type, column_constraints)
	}

	fn drop_column(&self, table: &str, field: &str) -> String {
		format!("ALTER TABLE \"{}\" DROP COLUMN \"{}\";", table, field)
	}

	fn update_column(&self, table: &str, field: &FieldMeta) -> String {
		let field_type = Self::entity_type_to_column_type(field.ty);
		format!("ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {};", table, field.name, field_type)
	}

	fn add_identity_generation(&self, table: &str, field: &FieldMeta) -> String {
		let identity_generation = Self::identity_generation(field.identity_generation.unwrap());
		format!(
			"ALTER TABLE \"{}\" ALTER COLUMN \"{}\" ADD GENERATED {} AS IDENTITY;",
			table, field.name, identity_generation
		)
	}

	fn entity_type_to_column_type(ty: FieldType) -> &'static str {
		match ty {
			FieldType::I32 => "integer",
			FieldType::String => "character varying",
		}
	}

	fn make_column_constraints(field: &FieldMeta) -> String {
		let mut column_constraints = vec![];
		if field.optional {
			column_constraints.push("NULL".into());
		} else {
			column_constraints.push("NOT NULL".into());
		}
		if let Some(identity_generation) = field.identity_generation {
			let identity_generation = Self::identity_generation(identity_generation);
			column_constraints.push(format!("GENERATED {} AS IDENTITY", identity_generation));
		}
		column_constraints.join(" ")
	}

	fn identity_generation(identity_generation: IdentityGeneration) -> &'static str {
		match identity_generation {
			IdentityGeneration::Always => "ALWAYS",
			IdentityGeneration::ByDefault => "BY DEFAULT",
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
					let field_meta = &entity.fields[column];
					if let Some(old_column) = old_fields.get(column) {
						// update columns
						if old_column.ty != Self::entity_type_to_column_type(field_meta.ty) {
							up.push(self.update_column(table, &field_meta));
							// TODO: detect when we can reverse this update, such as when shrinking an integer type
						}
						if old_column.identity_generation != field_meta.identity_generation {
							if field_meta.identity_generation.is_some() {
								up.push(self.add_identity_generation(table, &field_meta));
							} else {
								unimplemented!("removing identity generation is not supported yet {:?}", field_meta);
							}
						}
					} else {
						// create columns
						let column = &field_meta;
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
		identity_generation: Option<String>,
	}
	let fields_query = query_as::<_, FieldRow>(
		"SELECT table_name, column_name, data_type, identity_generation
		 identity_generation
		FROM information_schema.columns
		WHERE table_schema = 'public' AND table_name <> '_sqlx_migrations';",
	);
	let fields = fields_query
		.fetch_all(pool)
		.await?
		.into_iter()
		.map(|x| {
			(
				x.table_name,
				(x.column_name.clone(), PgField {
					name: x.column_name,
					ty: x.data_type,
					identity_generation: x.identity_generation.map(|x| IdentityGeneration::from_name(&x)),
				}),
			)
		})
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
	identity_generation: Option<IdentityGeneration>,
}
