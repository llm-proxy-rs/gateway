use anyhow::Context;
use sqlx::PgPool;

pub struct UsageStats {
    pub usage_count: i64,
    pub total_tokens: i64,
}

pub async fn create_user(pool: &PgPool, email: &str) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO users (email) VALUES ($1)")
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_usage_stats(pool: &PgPool, email: &str) -> anyhow::Result<UsageStats> {
    let result = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as usage_count,
            COALESCE(SUM(total_tokens), 0)::bigint as total_tokens
        FROM
            usage u
        JOIN
            users usr ON u.user_id = usr.user_id
        WHERE
            usr.email = $1
            AND date_trunc('month', u.created_at) = date_trunc('month', now())
        "#,
        email
    )
    .fetch_one(pool)
    .await?;

    Ok(UsageStats {
        usage_count: result.usage_count.context("usage_count is null")?,
        total_tokens: result.total_tokens.context("total_tokens is null")?,
    })
}
