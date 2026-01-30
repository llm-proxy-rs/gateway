use sqlx::PgPool;

pub async fn create_user(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "INSERT INTO users (email) VALUES ($1)",
        email.to_lowercase()
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn toggle_user_usage_tracking_enabled(
    pool: &PgPool,
    user_email: &str,
) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE users SET usage_tracking_enabled = NOT usage_tracking_enabled WHERE email = $1",
        user_email.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_user_usage_tracking_enabled(
    pool: &PgPool,
    user_email: &str,
) -> anyhow::Result<bool> {
    let user_usage_tracking_enabled = sqlx::query_scalar!(
        "SELECT usage_tracking_enabled FROM users WHERE email = $1",
        user_email.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(user_usage_tracking_enabled)
}
