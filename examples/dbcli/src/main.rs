mod migrate;

use anyhow::{Error, Result};
use clap::{Parser, Subcommand};
use dotenv::dotenv;
use migrate::migrate;

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

#[tokio::main]
async fn main() -> Result<(), Error> {
	env_logger::init();

	let cli = Cli::parse();

	dotenv().ok();

	match &cli.command {
		Commands::Migrate { name } => migrate(name).await?,
	}

	Ok(())
}
