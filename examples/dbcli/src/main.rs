use anyhow::Result;
use schema::ENTITIES;

#[tokio::main]
async fn main() -> Result<()> {
	nice_orm_cli::run(&ENTITIES).await?;
	Ok(())
}
