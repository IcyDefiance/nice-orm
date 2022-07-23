pub mod postgres;

use anyhow::Result;
use async_trait::async_trait;
use sqlx::migrate::Migrator;

#[async_trait]
pub trait SqlGen {
	async fn gen_migration(&self) -> Result<(String, Option<String>)>;
	async fn run_migrations(&self, migrator: &Migrator) -> Result<()>;
}
