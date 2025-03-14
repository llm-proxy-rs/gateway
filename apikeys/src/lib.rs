use anyhow::Context;
use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn create_api_key(pool: &PgPool, user_email: &str) -> anyhow::Result<Uuid> {
    let api_key = Uuid::new_v4();

    sqlx::query!(
        r#"
        INSERT INTO api_keys (api_key, user_id)
        SELECT $1, user_id FROM users WHERE email = $2
        "#,
        api_key.to_string(),
        user_email
    )
    .execute(pool)
    .await?;

    Ok(api_key)
}

async fn api_key_exists(pool: &PgPool, api_key: &str) -> anyhow::Result<bool> {
    let record = sqlx::query!(
        r#"
        SELECT COUNT(*) as count FROM api_keys WHERE api_key = $1
        "#,
        api_key
    )
    .fetch_one(pool)
    .await?;

    Ok(record.count.is_some_and(|count| count > 0))
}

pub async fn validate_api_key(pool: &PgPool, headers: &HeaderMap) -> anyhow::Result<bool> {
    let api_key = headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|auth| {
            if auth.starts_with("Bearer ") {
                Some(auth.trim_start_matches("Bearer ").trim())
            } else {
                None
            }
        })
        .context("Missing API key")?;

    api_key_exists(pool, api_key).await
}
