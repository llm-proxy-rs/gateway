use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, sse::Sse},
};
use chat::provider::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use inference_profiles::create_inference_profile;
use myerrors::AppError;
use myhandlers::AppState;
use request::ChatCompletionsRequest;
use tracing::{debug, error};

use crate::{
    handlers::usage_callback::create_usage_callback,
    validation::check_api_key_exists_and_model_exists_and_get_inference_profile_arn,
};

#[allow(dead_code)]
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
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "Invalid or missing API key"))?;

    let (api_key_exists, model_exists, inference_profile_arn) =
        check_api_key_exists_and_model_exists_and_get_inference_profile_arn(
            &state.db_pool,
            &api_key,
            &payload.model,
        )
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

    if payload.stream == Some(false) {
        error!("Streaming is required but was disabled by client (stream: false)");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    let model_name = if let Some(inference_profile_arn) = inference_profile_arn {
        inference_profile_arn
    } else {
        create_inference_profile(
            &state.db_pool,
            &api_key,
            &payload.model,
            &state.aws_region,
            &state.aws_account_id,
            &state.inference_profile_prefixes,
        )
        .await
        .unwrap_or(payload.model.to_lowercase())
    };

    let usage_callback = create_usage_callback(&model_name);

    payload.model = model_name;

    let stream = BedrockChatCompletionsProvider::new(state.bedrockruntime_client.clone())
        .chat_completions_stream(payload, usage_callback)
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}
