use std::path::Path;

use anyhow::Result;
use schema::ENTITIES;

#[tokio::main]
async fn main() -> Result<()> {
	nice_orm_cli::run(Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations"), &ENTITIES).await?;
	Ok(())
}
