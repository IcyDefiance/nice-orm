pub mod postgres;

use anyhow::Result;
use async_trait::async_trait;
use sqlx::migrate::Migrator;

#[async_trait]
pub trait SqlGen {}
