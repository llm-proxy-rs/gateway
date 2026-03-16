use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::DateTime;
use myhandlers::{AppState, ModelInfo, ModelsResponse};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ModelsQuery {
    pub limit: Option<usize>,
    pub after_id: Option<String>,
    pub before_id: Option<String>,
}

pub async fn v1_models(
    State(state): State<AppState>,
    Query(models_query): Query<ModelsQuery>,
) -> impl IntoResponse {
    let limit = models_query.limit.unwrap_or(20).min(1000);

    let model_infos: Vec<ModelInfo> = state
        .model_mapping
        .get_model_configs()
        .iter()
        .map(|model_config| ModelInfo {
            id: model_config.anthropic_model_id.clone(),
            display_name: model_config.anthropic_display_name.clone(),
            created_at: DateTime::UNIX_EPOCH,
            model_type: "model".to_string(),
        })
        .collect();

    let mut start = 0;
    let mut end = model_infos.len();

    if let Some(ref after_id) = models_query.after_id {
        if let Some(idx) = model_infos.iter().position(|model_info| model_info.id == *after_id) {
            start = idx + 1;
        }
    }
    if let Some(ref before_id) = models_query.before_id {
        if let Some(idx) = model_infos.iter().position(|model_info| model_info.id == *before_id) {
            end = idx;
        }
    }

    let filtered_model_infos = if start < end {
        &model_infos[start..end]
    } else {
        &[]
    };

    let page = &filtered_model_infos[..filtered_model_infos.len().min(limit)];
    let has_more = filtered_model_infos.len() > limit;

    let models_response = ModelsResponse {
        first_id: page.first().map(|model_info| model_info.id.clone()),
        last_id: page.last().map(|model_info| model_info.id.clone()),
        has_more,
        data: page.to_vec(),
    };

    (StatusCode::OK, Json(models_response))
}
