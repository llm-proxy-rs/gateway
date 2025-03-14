use sqlx::PgPool;

pub async fn create_user(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO users (email) VALUES ($1)")
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}
