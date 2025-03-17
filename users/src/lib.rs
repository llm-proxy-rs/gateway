use sqlx::PgPool;

pub async fn create_user(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO users (email) VALUES ($1)")
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_total_spent(pool: &PgPool, email: &str) -> anyhow::Result<f64> {
    let result = sqlx::query!(
        r#"
        SELECT 
            total_spent
        FROM users
        WHERE email = $1
        "#,
        email
    )
    .fetch_one(pool)
    .await?;

    Ok(result.total_spent)
}
