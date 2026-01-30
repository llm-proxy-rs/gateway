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

pub async fn get_user_usage_count_and_user_total_tokens(
    pool: &PgPool,
    email: &str,
) -> anyhow::Result<(i64, i64)> {
    let result = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as "user_usage_count!",
            COALESCE(SUM(total_tokens), 0)::bigint as "user_total_tokens!"
        FROM
            usage u
        JOIN
            users usr ON u.user_id = usr.user_id
        WHERE
            usr.email = $1
            AND date_trunc('month', u.created_at) = date_trunc('month', now())
        "#,
        email.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((result.user_usage_count, result.user_total_tokens))
}

pub async fn update_user_usage_record(pool: &PgPool, user_email: &str) -> anyhow::Result<()> {
    sqlx::query!(
        "UPDATE users SET usage_record = NOT usage_record WHERE email = $1",
        user_email.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_user_usage_record(pool: &PgPool, user_email: &str) -> anyhow::Result<bool> {
    let usage_record = sqlx::query_scalar!(
        "SELECT usage_record FROM users WHERE email = $1",
        user_email.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(usage_record)
}
