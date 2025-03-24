use anyhow::Context;
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
        WITH updated_user AS (
            UPDATE users
            SET
                total_spent = 0,
                updated_at = now()
            WHERE
                email = $1
                AND date_trunc('month', updated_at) <> date_trunc('month', now())
            RETURNING total_spent
        )
        SELECT
            COALESCE(
                (SELECT total_spent FROM updated_user),
                (SELECT total_spent FROM users WHERE email = $1 LIMIT 1)
            ) as total_spent
        "#,
        email
    )
    .fetch_one(pool)
    .await?;

    result.total_spent.context("total_spent is null")
}
