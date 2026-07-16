use sqlx::PgPool;

pub fn is_openai_model(model_name: &str) -> bool {
    let model_name = model_name.to_ascii_lowercase();
    model_name.starts_with("openai.") || model_name.starts_with("gpt-")
}

pub fn normalize_openai_model_id(model_name: &str) -> String {
    if model_name.to_ascii_lowercase().starts_with("gpt-") {
        format!("openai.{model_name}")
    } else {
        model_name.to_string()
    }
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
        assert!(is_openai_model("gpt-5.6-sol"));
        assert!(is_openai_model("GPT-5.6-sol"));
    }

    #[test]
    fn claude_models_are_not_openai() {
        assert!(!is_openai_model("us.anthropic.claude-sonnet-4-6"));
        assert!(!is_openai_model("global.anthropic.claude-sonnet-5"));
        assert!(!is_openai_model("claude-opus-4-6"));
    }

    #[test]
    fn codex_bare_gpt_ids_get_openai_prefix() {
        assert_eq!(
            normalize_openai_model_id("gpt-5.6-sol"),
            "openai.gpt-5.6-sol"
        );
        assert_eq!(
            normalize_openai_model_id("gpt-5.6-luna"),
            "openai.gpt-5.6-luna"
        );
        assert_eq!(
            normalize_openai_model_id("GPT-5.6-sol"),
            "openai.GPT-5.6-sol"
        );
    }

    #[test]
    fn already_prefixed_gpt_ids_pass_through() {
        assert_eq!(
            normalize_openai_model_id("openai.gpt-5.6-sol"),
            "openai.gpt-5.6-sol"
        );
    }

    #[test]
    fn non_openai_ids_pass_through_unchanged() {
        assert_eq!(
            normalize_openai_model_id("us.anthropic.claude-sonnet-4-6"),
            "us.anthropic.claude-sonnet-4-6"
        );
        assert_eq!(
            normalize_openai_model_id("claude-opus-4-6"),
            "claude-opus-4-6"
        );
    }
}
