use anthropic_request::V1MessagesRequest;
use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::sse::Sse,
};
use chat::provider::{BedrockV1MessagesProvider, V1MessagesProvider};
use futures::Stream;
use myerrors::AppError;
use myhandlers::AppState;
use tracing::{debug, error};

use crate::validation::check_api_key_and_model;

use super::usage_callback::create_usage_callback;

pub async fn v1_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<V1MessagesRequest>,
) -> Result<
    (
        StatusCode,
        Sse<impl Stream<Item = Result<axum::response::sse::Event, anyhow::Error>>>,
    ),
    AppError,
> {
    debug!("Received v1/messages request for model: {}", payload.model);

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key in Authorization header")?;

    payload.model = payload.model.to_lowercase();

    let validation = check_api_key_and_model(&state.db_pool, &api_key, &payload.model).await?;

    if !validation.api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    if !validation.model_exists {
        error!("Model name validation failed: Invalid model name");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing model name"
        )));
    }

    if payload.stream != Some(true) {
        error!("Streaming is required but was disabled");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    let usage_callback = create_usage_callback(
        state.db_pool.clone(),
        api_key.clone(),
        payload.model.clone(),
    );

    let stream = BedrockV1MessagesProvider::new()
        .await
        .v1_messages_stream(payload, usage_callback)
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}
