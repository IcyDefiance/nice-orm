mod migrate;

use std::path::Path;

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use migrate::migrate;
use nice_orm::entity_meta::Entities;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[clap(propagate_version = true)]
struct Cli {
	#[clap(subcommand)]
	command: Commands,
}

#[derive(Subcommand)]
enum Commands {
	Migrate { name: String },
}

pub async fn run(migration_dir: impl AsRef<Path>, entities: Entities) -> Result<(), Error> {
	env_logger::init();

	let cli = Cli::parse();

	dotenv().ok();

	match &cli.command {
		Commands::Migrate { name } => migrate(migration_dir, entities, name).await?,
	}

	Ok(())
}
