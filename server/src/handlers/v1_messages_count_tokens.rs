use anthropic_request::V1MessagesCountTokensRequest;
use anthropic_response::V1MessagesCountTokensResponse;
use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chat::provider::{BedrockV1MessagesProvider, V1MessagesProvider};
use myerrors::AppError;
use myhandlers::AppState;
use tracing::{error, info};

use crate::validation::check_api_key_exists_and_model_exists;

pub async fn v1_messages_count_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<V1MessagesCountTokensRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!(
        "Received Anthropic v1/messages/count_tokens request for model: {}",
        payload.model
    );

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key (provide Authorization: Bearer <key> or x-api-key header)")?;

    let (api_key_exists, model_exists) =
        check_api_key_exists_and_model_exists(&state.db_pool, &api_key, &payload.model).await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    if !model_exists {
        error!("Model name validation failed: Invalid model name");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing model name"
        )));
    }

    payload.model = payload.model.to_lowercase();

    let provider = BedrockV1MessagesProvider::new().await;
    let count = provider
        .v1_messages_count_tokens(&payload, &state.inference_profile_prefixes)
        .await?;

    Ok((
        StatusCode::OK,
        Json(V1MessagesCountTokensResponse {
            input_tokens: count,
        }),
    ))
}
