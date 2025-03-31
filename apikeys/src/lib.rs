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
        user_email
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
        user_email
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn get_api_key(headers: &HeaderMap) -> Option<String> {
    headers
        .get("Authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|auth| {
            if auth.starts_with("Bearer ") {
                Some(auth.trim_start_matches("Bearer ").trim().to_string())
            } else {
                None
            }
        })
}
