use apikeys::get_api_key;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode, header::CONTENT_TYPE},
    response::Response,
};
use chat::provider::{MantleV1ResponsesProvider, V1ResponsesProvider};
use myerrors::AppError;
use myhandlers::AppState;
use projects::create_project;
use tracing::{debug, error, info};

use crate::{
    handlers::usage_callback::create_usage_callback,
    validation::{check_api_key_exists_and_model_exists_and_get_project_id, is_openai_model},
};

/// Transparent passthrough to Bedrock Mantle's OpenAI Responses API.
///
/// Resolves or creates a per-(user, model) Bedrock project (tagged like
/// application inference profiles) and injects `OpenAI-Project`.
pub async fn v1_responses(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AppError> {
    let model = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value
                .get("model")
                .and_then(|m| m.as_str())
                .map(str::to_owned)
        })
        .ok_or_else(|| AppError::new(StatusCode::BAD_REQUEST, "Missing or invalid model"))?;

    debug!("Received v1/responses request for model: {}", model);

    if !is_openai_model(&model) {
        error!(
            "Claude/Anthropic model '{}' is not supported on /v1/responses; use /v1/messages",
            model
        );
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "Claude models are only supported on /v1/messages",
        ));
    }

    let api_key = get_api_key(&headers)
        .await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "Invalid or missing API key"))?;

    let (api_key_exists, model_exists, existing_project_id) =
        check_api_key_exists_and_model_exists_and_get_project_id(&state.db_pool, &api_key, &model)
            .await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "Invalid or missing API key",
        ));
    }

    if !model_exists {
        error!("Model name validation failed: Invalid model name");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing model name"
        )));
    }

    let project_id = if let Some(project_id) = existing_project_id {
        project_id
    } else {
        create_project(
            &state.db_pool,
            &state.http_client,
            &state.credentials_provider,
            &api_key,
            &model,
            &state.aws_region,
        )
        .await?
    };

    info!(
        "Proxying OpenAI Responses API request for model: {}, project: {}",
        model, project_id
    );

    let usage_callback = create_usage_callback(&model);
    let provider = MantleV1ResponsesProvider::new(
        state.http_client.clone(),
        state.aws_region.clone(),
        state.credentials_provider.clone(),
    );
    let upstream = provider
        .v1_responses_stream(body.to_vec(), Some(&project_id), usage_callback)
        .await?;

    let status = StatusCode::from_u16(upstream.status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);

    if !status.is_success() {
        error!("Bedrock Mantle Responses request returned {}", status);
    }

    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, upstream.content_type)
        .body(Body::from_stream(upstream.body))
        .map_err(|e| anyhow::anyhow!("Failed to build Responses proxy response: {}", e).into())
}
