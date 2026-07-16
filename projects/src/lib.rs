use anyhow::Result;
use aws_credential_types::provider::{ProvideCredentials, SharedCredentialsProvider};
use aws_sigv4::http_request::{
    PayloadChecksumKind, SignableBody, SignableRequest, SigningParams, SigningSettings, sign,
};
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use serde::Deserialize;
use sqlx::PgPool;
use std::time::SystemTime;
use tracing::{error, info};
use uuid::Uuid;

const PROJECTS_PATH: &str = "/v1/organization/projects";

fn bedrock_mantle_url(region: &str, path: &str) -> String {
    format!("https://bedrock-mantle.{region}.api.aws{path}")
}

async fn sign_bedrock_json_post(
    credentials_provider: &SharedCredentialsProvider,
    region: &str,
    url: &str,
    body: &[u8],
) -> Result<Vec<(String, String)>> {
    let credentials = credentials_provider
        .provide_credentials()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to resolve AWS credentials: {}", e))?;
    let identity: Identity = credentials.into();

    let mut signing_settings = SigningSettings::default();
    signing_settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;

    let signing_params: SigningParams = v4::SigningParams::builder()
        .identity(&identity)
        .region(region)
        .name("bedrock")
        .time(SystemTime::now())
        .settings(signing_settings)
        .build()?
        .into();

    let signable_request = SignableRequest::new(
        "POST",
        url,
        std::iter::once(("content-type", "application/json")),
        SignableBody::Bytes(body),
    )?;

    let (instructions, _signature) = sign(signable_request, &signing_params)?.into_parts();

    Ok(instructions
        .headers()
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect())
}

#[derive(Debug, Deserialize)]
struct CreateProjectResponse {
    arn: String,
    id: String,
    name: String,
}

/// Creates a Bedrock Mantle project tagged with GatewayUserId / GatewayModelId
/// (mirroring application inference profiles) and stores a per-(user, model) ref.
pub async fn create_project(
    pool: &PgPool,
    http_client: &reqwest::Client,
    credentials_provider: &SharedCredentialsProvider,
    api_key: &str,
    model_name: &str,
    aws_region: &str,
) -> Result<String> {
    let ids = sqlx::query!(
        r#"
        SELECT
            (SELECT user_id FROM api_keys WHERE api_key = $1) AS user_id,
            (SELECT model_id FROM models WHERE model_name = $2 AND is_disabled = FALSE) AS model_id
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase(),
    )
    .fetch_one(pool)
    .await?;

    let user_id = ids.user_id.ok_or_else(|| {
        error!("API key not found");
        anyhow::anyhow!("API key not found")
    })?;
    let model_id = ids.model_id.ok_or_else(|| {
        error!(%model_name, "Model not found");
        anyhow::anyhow!("Model not found")
    })?;

    let project_name = Uuid::new_v4().to_string();
    let body = serde_json::json!({
        "name": project_name,
        "tags": {
            "GatewayUserId": user_id.to_string(),
            "GatewayModelId": model_id.to_string(),
        }
    });
    let body_bytes = serde_json::to_vec(&body)?;

    let url = bedrock_mantle_url(aws_region, PROJECTS_PATH);
    let signed_headers =
        sign_bedrock_json_post(credentials_provider, aws_region, &url, &body_bytes).await?;

    let mut request = http_client
        .post(&url)
        .header("content-type", "application/json");
    for (name, value) in signed_headers {
        request = request.header(name, value);
    }

    let response = request.body(body_bytes).send().await.map_err(|e| {
        error!(
            "Failed to create Bedrock project '{}': {:?}",
            project_name, e
        );
        e
    })?;

    let status = response.status();
    let response_body = response.bytes().await?;
    if !status.is_success() {
        let message = String::from_utf8_lossy(&response_body);
        error!(
            project_name = %project_name,
            %status,
            upstream_body = %message,
            "Failed to create Bedrock project"
        );
        anyhow::bail!("Failed to create Bedrock project");
    }

    let created: CreateProjectResponse = serde_json::from_slice(&response_body)?;

    sqlx::query!(
        r#"
        INSERT INTO projects (user_id, model_id, openai_project_id, project_arn, project_name)
        SELECT ak.user_id, m.model_id, $3, $4, $5
        FROM api_keys ak, models m
        WHERE ak.api_key = $1 AND m.model_name = $2 AND m.is_disabled = FALSE
        ON CONFLICT (user_id, model_id) DO NOTHING
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase(),
        &created.id,
        &created.arn,
        &created.name,
    )
    .execute(pool)
    .await?;

    // Prefer the persisted project when a concurrent create already won.
    let openai_project_id = sqlx::query_scalar!(
        r#"
        SELECT openai_project_id
        FROM projects
        WHERE user_id = (SELECT user_id FROM api_keys WHERE api_key = $1)
          AND model_id = (SELECT model_id FROM models WHERE model_name = $2 AND is_disabled = FALSE)
        LIMIT 1
        "#,
        api_key.to_lowercase(),
        model_name.to_lowercase(),
    )
    .fetch_optional(pool)
    .await?
    .unwrap_or(created.id);

    info!(
        "Created and stored project: {} (id: {}, arn: {})",
        project_name, openai_project_id, created.arn
    );

    Ok(openai_project_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bedrock_mantle_projects_url() {
        assert_eq!(
            bedrock_mantle_url("us-east-1", PROJECTS_PATH),
            "https://bedrock-mantle.us-east-1.api.aws/v1/organization/projects"
        );
    }
}
