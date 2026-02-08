use anyhow::Result;
use aws_sdk_bedrock::types::InferenceProfileModelSource;
use sqlx::PgPool;
use tracing::{error, info};
use uuid::Uuid;

pub async fn create_inference_profile(
    pool: &PgPool,
    api_key: &str,
    model_name: &str,
    aws_region: &str,
    aws_account_id: &str,
    inference_profile_prefixes: &[String],
) -> Result<String> {
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = aws_sdk_bedrock::Client::new(&config);

    let copy_from = if inference_profile_prefixes
        .iter()
        .any(|inference_profile_prefix| model_name.starts_with(inference_profile_prefix.as_str()))
    {
        format!("arn:aws:bedrock:{aws_region}:{aws_account_id}:inference-profile/{model_name}")
    } else {
        model_name.to_string()
    };

    let inference_profile_name = Uuid::new_v4().to_string();

    let response = client
        .create_inference_profile()
        .inference_profile_name(&inference_profile_name)
        .model_source(InferenceProfileModelSource::CopyFrom(copy_from))
        .send()
        .await
        .map_err(|e| {
            error!(
                "Failed to create inference profile '{}': {:?}",
                inference_profile_name, e
            );
            e
        })?;

    let inference_profile_arn = response.inference_profile_arn().to_string();

    sqlx::query!(
        r#"
        INSERT INTO inference_profiles (user_id, model_id, inference_profile_arn, inference_profile_name)
        SELECT ak.user_id, m.model_id, $3, $4
        FROM api_keys ak, models m
        WHERE ak.api_key = $1 AND m.model_name = $2
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase(),
        &inference_profile_arn.to_lowercase(),
        &inference_profile_name.to_lowercase(),
    )
    .execute(pool)
    .await?;

    info!(
        "Created and stored inference profile: {} (ARN: {})",
        inference_profile_name, inference_profile_arn
    );

    Ok(inference_profile_arn)
}
