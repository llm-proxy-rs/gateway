use sqlx::PgPool;

pub async fn create_user(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "INSERT INTO users (user_email) VALUES ($1)",
        email.to_lowercase()
    )
    .execute(pool)
    .await?;
    Ok(())
}
