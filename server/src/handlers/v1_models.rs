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
        .ok_or_else(|| AppError::new(StatusCode::UNAUTHORIZED, "Invalid or missing API key"))?;

    let api_key_exists = check_api_key_exists(&state.db_pool, &api_key).await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::new(
            StatusCode::UNAUTHORIZED,
            "Invalid or missing API key",
        ));
    }

    let model_infos: Vec<ModelInfo> = state
        .model_configs
        .iter()
        .map(|model_config| ModelInfo {
            id: model_config.anthropic_model_id.clone(),
            display_name: model_config.anthropic_display_name.clone(),
            created_at: DateTime::UNIX_EPOCH,
            type_: "model".to_string(),
        })
        .collect();

    let models_response = ModelsResponse {
        first_id: model_infos.first().map(|m| m.id.clone()),
        last_id: model_infos.last().map(|m| m.id.clone()),
        has_more: false,
        data: model_infos,
    };

    Ok((StatusCode::OK, Json(models_response)))
}
