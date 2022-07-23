mod sql_gen;

use self::sql_gen::{postgres::PostgresSqlGen, SqlGen};
use anyhow::Result;
use chrono::Utc;
use sqlx::migrate::Migrator;
use std::{env, path::Path};
use tokio::{
	fs::{self, File},
	io::AsyncWriteExt,
};

pub async fn migrate(name: &str) -> Result<()> {
	let migration_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
	let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is not set");
	let sql_gen = match database_url.split(':').next().unwrap() {
		"postgres" => Box::new(PostgresSqlGen::new(&database_url).await?),
		_ => panic!("Unsupported database"),
	};

	sql_gen.run_migrations(&make_migrator(&migration_dir).await?).await?;

	let (up, down) = sql_gen.gen_migration().await?;

	fs::create_dir_all(&migration_dir).await?;
	if up.len() > 0 {
		let now = Utc::now().format("%Y%m%d%H%M%S");
		let migration_name = format!("{}_{}", now, name);

		let up_filename = format!("{}{}.sql", migration_name, down.as_ref().map(|_| ".up").unwrap_or(""));
		let mut up_file = File::create(migration_dir.join(up_filename)).await?;
		up_file.write_all(up.as_bytes()).await?;

		if let Some(down) = down {
			if down.len() > 0 {
				let mut down_file = File::create(migration_dir.join(format!("{}.down.sql", migration_name))).await?;
				down_file.write_all(down.as_bytes()).await?;
			}
		}

		sql_gen.run_migrations(&make_migrator(&migration_dir).await?).await?;
	} else {
		println!("No schema changes detected");
	}

	Ok(())
}

async fn make_migrator(migration_dir: impl AsRef<Path>) -> Result<Migrator> {
	Ok(Migrator::new(migration_dir.as_ref()).await?)
}
