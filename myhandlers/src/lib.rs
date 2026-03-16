use aws_sdk_bedrockruntime::Client;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};
use handlers::CallbackQuery;
use myerrors::AppError;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};
use tower_sessions::Session;

// ── Model mapping (config-driven) ────────────────────────────────

#[derive(Clone, Debug, Deserialize)]
pub struct ModelConfig {
    pub anthropic_model_id: String,
    pub anthropic_display_name: String,
    pub bedrock_model_id: String,
}

#[derive(Clone, Debug)]
pub struct ModelMapping {
    model_configs: Vec<ModelConfig>,
    anthropic_to_bedrock: HashMap<String, String>,
}

impl ModelMapping {
    pub fn new(model_configs: Vec<ModelConfig>) -> Self {
        let anthropic_to_bedrock = model_configs
            .iter()
            .map(|model_config| (model_config.anthropic_model_id.clone(), model_config.bedrock_model_id.clone()))
            .collect();
        Self {
            model_configs,
            anthropic_to_bedrock,
        }
    }

    /// Returns the Bedrock model ID for a given Anthropic model ID.
    /// If no mapping exists, returns the original ID as-is (passthrough).
    pub fn get_bedrock_model_id(&self, anthropic_model_id: &str) -> String {
        self.anthropic_to_bedrock
            .get(anthropic_model_id)
            .cloned()
            .unwrap_or_else(|| anthropic_model_id.to_string())
    }

    pub fn get_model_configs(&self) -> &[ModelConfig] {
        &self.model_configs
    }
}

// ── /v1/models response types ────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub created_at: DateTime<Utc>,
    #[serde(rename = "type")]
    pub model_type: String,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub data: Vec<ModelInfo>,
    pub first_id: Option<String>,
    pub last_id: Option<String>,
    pub has_more: bool,
}

// ── AppState ─────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub anthropic_beta_whitelist: Vec<String>,
    pub aws_account_id: String,
    pub aws_region: String,
    pub bedrockruntime_client: Client,
    pub cognito_client_id: String,
    pub cognito_client_secret: String,
    pub cognito_domain: String,
    pub cognito_redirect_uri: String,
    pub cognito_region: String,
    pub cognito_user_pool_id: String,
    pub db_pool: Arc<PgPool>,
    pub inference_profile_prefixes: Vec<String>,
    pub model_mapping: ModelMapping,
}

pub async fn logout(session: Session) -> Result<Response, AppError> {
    session.delete().await?;
    Ok(Redirect::to("/").into_response())
}

pub async fn login(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let state = State(handlers::AppState {
        client_id: state.cognito_client_id.clone(),
        client_secret: state.cognito_client_secret.clone(),
        domain: state.cognito_domain.clone(),
        redirect_uri: state.cognito_redirect_uri.clone(),
        region: state.cognito_region.clone(),
        user_pool_id: state.cognito_user_pool_id.clone(),
    });
    Ok(handlers::login(session, state).await?)
}

pub async fn callback(
    query: Query<CallbackQuery>,
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let state = State(handlers::AppState {
        client_id: state.cognito_client_id.clone(),
        client_secret: state.cognito_client_secret.clone(),
        domain: state.cognito_domain.clone(),
        redirect_uri: state.cognito_redirect_uri.clone(),
        region: state.cognito_region.clone(),
        user_pool_id: state.cognito_user_pool_id.clone(),
    });
    Ok(handlers::callback(query, session, state).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_model_mapping() -> ModelMapping {
        ModelMapping::new(vec![
            ModelConfig {
                anthropic_model_id: "claude-opus-4-6".to_string(),
                anthropic_display_name: "Claude Opus 4.6".to_string(),
                bedrock_model_id: "us.anthropic.claude-opus-4-6-v1".to_string(),
            },
            ModelConfig {
                anthropic_model_id: "claude-sonnet-4-6".to_string(),
                anthropic_display_name: "Claude Sonnet 4.6".to_string(),
                bedrock_model_id: "us.anthropic.claude-sonnet-4-6".to_string(),
            },
        ])
    }

    #[test]
    fn get_bedrock_model_id_returns_mapped_id() {
        let model_mapping = build_model_mapping();
        assert_eq!(
            model_mapping.get_bedrock_model_id("claude-opus-4-6"),
            "us.anthropic.claude-opus-4-6-v1"
        );
        assert_eq!(
            model_mapping.get_bedrock_model_id("claude-sonnet-4-6"),
            "us.anthropic.claude-sonnet-4-6"
        );
    }

    #[test]
    fn get_bedrock_model_id_passes_through_unmapped_id() {
        let model_mapping = build_model_mapping();
        assert_eq!(
            model_mapping.get_bedrock_model_id("us.anthropic.claude-3-haiku-20240307-v1:0"),
            "us.anthropic.claude-3-haiku-20240307-v1:0"
        );
    }

    #[test]
    fn get_model_configs_returns_all_configs() {
        let model_mapping = build_model_mapping();
        let model_configs = model_mapping.get_model_configs();
        assert_eq!(model_configs.len(), 2);
        assert_eq!(model_configs[0].anthropic_model_id, "claude-opus-4-6");
        assert_eq!(model_configs[1].anthropic_model_id, "claude-sonnet-4-6");
    }

    #[test]
    fn empty_model_mapping_passes_through_all_ids() {
        let model_mapping = ModelMapping::new(vec![]);
        assert_eq!(
            model_mapping.get_bedrock_model_id("claude-opus-4-6"),
            "claude-opus-4-6"
        );
        assert!(model_mapping.get_model_configs().is_empty());
    }

    #[test]
    fn anthropic_model_translates_and_preserves_response_model_id() {
        let model_mapping = build_model_mapping();
        let incoming_model = "claude-opus-4-6";

        let response_model_id = incoming_model.to_string();
        let bedrock_model_id = model_mapping.get_bedrock_model_id(incoming_model);

        assert_eq!(bedrock_model_id, "us.anthropic.claude-opus-4-6-v1");
        assert_eq!(response_model_id, "claude-opus-4-6");
    }

    #[test]
    fn bedrock_model_passes_through_and_preserves_response_model_id() {
        let model_mapping = build_model_mapping();
        let incoming_model = "us.anthropic.claude-haiku-4-5-20251001-v1:0";

        let response_model_id = incoming_model.to_string();
        let bedrock_model_id = model_mapping.get_bedrock_model_id(incoming_model);

        assert_eq!(bedrock_model_id, "us.anthropic.claude-haiku-4-5-20251001-v1:0");
        assert_eq!(response_model_id, "us.anthropic.claude-haiku-4-5-20251001-v1:0");
    }
}
