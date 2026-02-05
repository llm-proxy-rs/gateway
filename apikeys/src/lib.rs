use anyhow::Result;
use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_api_key(pool: &PgPool, user_email: &str) -> Result<Uuid> {
    let api_key = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO api_keys (api_key, user_id)
        SELECT $1, user_id FROM users WHERE email = $2
        "#,
        api_key.to_string(),
        user_email.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(api_key)
}

pub async fn disable_all_api_keys(pool: &PgPool, user_email: &str) -> Result<u64> {
    let result = sqlx::query!(
        r#"
        UPDATE api_keys
        SET is_disabled = TRUE, updated_at = now()
        WHERE user_id = (SELECT user_id FROM users WHERE email = $1)
        "#,
        user_email.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn get_api_keys_count_and_api_keys_count_active(
    pool: &PgPool,
    user_email: &str,
) -> Result<(i64, i64)> {
    let result = sqlx::query!(
        r#"
        SELECT
            COUNT(*) as "api_keys_count!",
            COUNT(*) FILTER (WHERE is_disabled = false) as "api_keys_count_active!"
        FROM api_keys
        WHERE user_id = (SELECT user_id FROM users WHERE email = $1)
        "#,
        user_email.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((result.api_keys_count, result.api_keys_count_active))
}

pub async fn get_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(token) = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        return Some(token.trim().to_string());
    }

    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
}
