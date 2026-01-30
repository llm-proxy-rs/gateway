use sqlx::PgPool;

pub struct ValidationResult {
    pub api_key_exists: bool,
    pub model_exists: bool,
}

pub async fn check_api_key_and_model(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<ValidationResult> {
    let result = sqlx::query_as!(
        ValidationResult,
        r#"
        SELECT
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE) as "api_key_exists!",
            EXISTS (SELECT 1 FROM models WHERE model_name = $2) as "model_exists!"
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(result)
}

pub async fn check_api_key_exists(pool: &PgPool, api_key: &str) -> anyhow::Result<bool> {
    let result = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE)
        "#,
        api_key.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok(result.unwrap_or(false))
}
