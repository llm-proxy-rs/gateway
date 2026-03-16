use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::DateTime;
use myerrors::AppError;
use myhandlers::{AppState, ModelInfo, ModelsResponse};
use tracing::error;

use crate::validation::check_api_key_exists;

pub async fn v1_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key (provide Authorization: Bearer <key> or x-api-key header)")?;

    let api_key_exists = check_api_key_exists(&state.db_pool, &api_key).await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    let data: Vec<ModelInfo> = state
        .model_configs
        .iter()
        .map(|model_config| ModelInfo {
            id: model_config.anthropic_model_id.clone(),
            display_name: model_config.anthropic_display_name.clone(),
            created_at: DateTime::UNIX_EPOCH,
            model_type: "model".to_string(),
        })
        .collect();

    let models_response = ModelsResponse {
        first_id: data.first().map(|m| m.id.clone()),
        last_id: data.last().map(|m| m.id.clone()),
        has_more: false,
        data,
    };

    Ok((StatusCode::OK, Json(models_response)))
}
