use anthropic_request::V1MessagesRequest;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, sse::Sse},
};
use chat::provider::{BedrockV1MessagesProvider, V1MessagesProvider};
use common::filter_anthropic_beta;
use inference_profiles::create_inference_profile;
use myerrors::AppError;
use myhandlers::{AppState, get_bedrock_model_id};
use tracing::{debug, error, info};

use crate::{
    handlers::usage_callback::create_usage_callback,
    validation::{
        check_api_key_exists_and_model_exists_and_get_inference_profile_arn, is_openai_model,
    },
};

pub async fn v1_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<V1MessagesRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!("Received v1/messages request for model: {}", payload.model);

    let api_key = get_api_key(&headers)
        .await
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "Invalid or missing API key"))?;

    let response_model_id = payload.model.clone();
    payload.model = get_bedrock_model_id(&state.anthropic_to_bedrock, &payload.model);

    if is_openai_model(&payload.model) {
        error!(
            "OpenAI model '{}' is not supported on /v1/messages; use /v1/responses",
            payload.model
        );
        return Err(AppError::new(
            StatusCode::BAD_REQUEST,
            "OpenAI models are only supported on /v1/responses",
        ));
    }

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

    let anthropic_beta = filter_anthropic_beta(&headers, &state.anthropic_beta_whitelist);
    info!("anthropic_beta: {:?}", anthropic_beta);

    payload.model = model_name;

    let provider = BedrockV1MessagesProvider::new(state.bedrockruntime_client.clone());

    if payload.stream == Some(true) {
        let stream = provider
            .v1_messages_stream(
                payload,
                Some(response_model_id),
                anthropic_beta,
                usage_callback,
            )
            .await?;
        return Ok((StatusCode::OK, Sse::new(stream)).into_response());
    }

    let message = provider
        .v1_messages(
            payload,
            Some(response_model_id),
            anthropic_beta,
            usage_callback,
        )
        .await?;
    Ok((StatusCode::OK, Json(message)).into_response())
}
