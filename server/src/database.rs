use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tracing::info;

pub async fn setup_database(database_url: &str) -> anyhow::Result<PgPool> {
    info!("Connecting to database");

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(3))
        .connect(database_url)
        .await?;

    info!("Database connection established");

    Ok(pool)
}
