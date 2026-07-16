use sqlx::PgPool;

/// Bedrock Mantle / OpenAI-compatible model IDs use the `openai.` prefix.
pub fn is_openai_model(model_name: &str) -> bool {
    model_name.to_ascii_lowercase().starts_with("openai.")
}

pub async fn check_api_key_exists_and_model_exists(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<(bool, bool)> {
    let result = sqlx::query!(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE) as "api_key_exists!",
            EXISTS (SELECT 1 FROM models WHERE model_name = $2 AND is_disabled = FALSE) as "model_exists!"
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((result.api_key_exists, result.model_exists))
}

pub async fn check_api_key_exists_and_model_exists_and_get_inference_profile_arn(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<(bool, bool, Option<String>)> {
    let result = sqlx::query!(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE) as "api_key_exists!",
            EXISTS (SELECT 1 FROM models WHERE model_name = $2 AND is_disabled = FALSE) as "model_exists!",
            (
                SELECT inference_profile_arn
                FROM inference_profiles
                WHERE user_id = (SELECT user_id FROM api_keys WHERE api_key = $1)
                  AND model_id = (SELECT model_id FROM models WHERE model_name = $2 AND is_disabled = FALSE)
                LIMIT 1
            ) as inference_profile_arn
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((
        result.api_key_exists,
        result.model_exists,
        result.inference_profile_arn,
    ))
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

pub async fn check_api_key_exists_and_model_exists_and_get_openai_project_id(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<(bool, bool, Option<String>)> {
    let result = sqlx::query!(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE) as "api_key_exists!",
            EXISTS (SELECT 1 FROM models WHERE model_name = $2 AND is_disabled = FALSE) as "model_exists!",
            (
                SELECT openai_project_id
                FROM projects
                WHERE user_id = (SELECT user_id FROM api_keys WHERE api_key = $1)
                  AND model_id = (SELECT model_id FROM models WHERE model_name = $2 AND is_disabled = FALSE)
                LIMIT 1
            ) as openai_project_id
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase()
    )
    .fetch_one(pool)
    .await?;

    Ok((
        result.api_key_exists,
        result.model_exists,
        result.openai_project_id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openai_models_are_detected_by_prefix() {
        assert!(is_openai_model("openai.gpt-5.6-sol"));
        assert!(is_openai_model("openai.gpt-5.6-luna"));
        assert!(is_openai_model("openai.gpt-5.6-terra"));
        assert!(is_openai_model("OpenAI.gpt-5.6-sol"));
    }

    #[test]
    fn claude_models_are_not_openai() {
        assert!(!is_openai_model("us.anthropic.claude-sonnet-4-6"));
        assert!(!is_openai_model("global.anthropic.claude-sonnet-5"));
        assert!(!is_openai_model("claude-opus-4-6"));
    }
}
