use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, sse::Sse},
};
use chat::bedrock::ReasoningEffortToThinkingBudgetTokens;
use chat::provider::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use myerrors::AppError;
use myhandlers::AppState;
use request::ChatCompletionsRequest;
use tracing::{debug, error};

use crate::validation::check_api_key_exists_and_model_exists;

use super::usage_callback::create_usage_callback;

pub async fn chat_completions(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut payload): Json<ChatCompletionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!(
        "Received chat completions request for model: {}",
        payload.model
    );

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key (provide Authorization: Bearer <key> or x-api-key header)")?;

    payload.model = payload.model.to_lowercase();

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

    if payload.stream == Some(false) {
        error!("Streaming is required but was disabled by client (stream: false)");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    let usage_callback = create_usage_callback(
        state.db_pool.clone(),
        api_key.clone(),
        payload.model.clone(),
    );

    let reasoning_effort_to_thinking_budget_tokens =
        ReasoningEffortToThinkingBudgetTokens::default();

    let stream = BedrockChatCompletionsProvider::new()
        .await
        .chat_completions_stream(
            payload,
            reasoning_effort_to_thinking_budget_tokens,
            usage_callback,
        )
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}
