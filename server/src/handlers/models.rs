use anyhow::Context;
use apikeys::get_api_key;
use axum::{Json, extract::State, http::HeaderMap, response::IntoResponse};
use models::{get_models, to_models_response};
use myerrors::AppError;
use myhandlers::AppState;

use crate::validation::check_api_key_exists;

pub async fn models(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key in Authorization header")?;

    let api_key_exists = check_api_key_exists(&state.db_pool, &api_key).await?;

    if !api_key_exists {
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    let models = get_models(&state.db_pool).await?;

    let models_response = to_models_response(&models);

    Ok(Json(models_response).into_response())
}
