use aws_credential_types::provider::SharedCredentialsProvider;
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

// ── Model config ─────────────────────────────────────────────────

fn default_max_input_tokens() -> u32 {
    200_000
}

fn default_max_tokens() -> u32 {
    64_000
}

#[derive(Clone, Debug, Deserialize)]
pub struct ModelConfig {
    pub anthropic_model_id: String,
    pub anthropic_display_name: String,
    pub bedrock_model_id: String,
    #[serde(default = "default_max_input_tokens")]
    pub max_input_tokens: u32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

impl From<&ModelConfig> for ModelInfo {
    fn from(config: &ModelConfig) -> Self {
        ModelInfo {
            id: config.anthropic_model_id.clone(),
            display_name: config.anthropic_display_name.clone(),
            max_input_tokens: config.max_input_tokens,
            max_tokens: config.max_tokens,
            supports1m: config.max_input_tokens >= 1_000_000,
            created_at: DateTime::UNIX_EPOCH,
            type_: "model".to_string(),
        }
    }
}

/// Returns the Bedrock model ID for a given Anthropic model ID.
/// If no mapping exists, returns the original ID as-is (passthrough).
pub fn get_bedrock_model_id(
    anthropic_to_bedrock: &HashMap<String, String>,
    anthropic_model_id: &str,
) -> String {
    anthropic_to_bedrock
        .get(anthropic_model_id)
        .cloned()
        .unwrap_or_else(|| anthropic_model_id.to_string())
}

// ── /v1/models response types ────────────────────────────────────

#[derive(Clone, Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub max_input_tokens: u32,
    pub max_tokens: u32,
    pub supports1m: bool,
    pub created_at: DateTime<Utc>,
    #[serde(rename = "type")]
    pub type_: String,
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
    pub credentials_provider: SharedCredentialsProvider,
    pub db_pool: Arc<PgPool>,
    pub http_client: reqwest::Client,
    pub inference_profile_prefixes: Vec<String>,
    pub anthropic_to_bedrock: HashMap<String, String>,
    pub model_configs: Vec<ModelConfig>,
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

    fn build_anthropic_to_bedrock() -> HashMap<String, String> {
        vec![
            (
                "claude-opus-4-6".to_string(),
                "us.anthropic.claude-opus-4-6-v1".to_string(),
            ),
            (
                "claude-sonnet-4-6".to_string(),
                "us.anthropic.claude-sonnet-4-6".to_string(),
            ),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn model_config_to_model_info_exposes_1m_context() {
        let config = ModelConfig {
            anthropic_model_id: "claude-sonnet-4-6".to_string(),
            anthropic_display_name: "Claude Sonnet 4.6".to_string(),
            bedrock_model_id: "us.anthropic.claude-sonnet-4-6".to_string(),
            max_input_tokens: 1_000_000,
            max_tokens: 64_000,
        };

        let info = ModelInfo::from(&config);

        assert_eq!(info.id, "claude-sonnet-4-6");
        assert_eq!(info.max_input_tokens, 1_000_000);
        assert_eq!(info.max_tokens, 64_000);
        assert!(info.supports1m);
        assert_eq!(info.type_, "model");
    }

    #[test]
    fn model_config_without_1m_context_sets_supports1m_false() {
        let config = ModelConfig {
            anthropic_model_id: "claude-haiku-4-5-20251001".to_string(),
            anthropic_display_name: "Claude Haiku 4.5".to_string(),
            bedrock_model_id: "us.anthropic.claude-haiku-4-5-20251001-v1:0".to_string(),
            max_input_tokens: 200_000,
            max_tokens: 64_000,
        };

        let info = ModelInfo::from(&config);

        assert!(!info.supports1m);
    }

    #[test]
    fn get_bedrock_model_id_returns_mapped_id() {
        let map = build_anthropic_to_bedrock();
        assert_eq!(
            get_bedrock_model_id(&map, "claude-opus-4-6"),
            "us.anthropic.claude-opus-4-6-v1"
        );
        assert_eq!(
            get_bedrock_model_id(&map, "claude-sonnet-4-6"),
            "us.anthropic.claude-sonnet-4-6"
        );
    }

    #[test]
    fn get_bedrock_model_id_passes_through_unmapped_id() {
        let map = build_anthropic_to_bedrock();
        assert_eq!(
            get_bedrock_model_id(&map, "us.anthropic.claude-3-haiku-20240307-v1:0"),
            "us.anthropic.claude-3-haiku-20240307-v1:0"
        );
    }

    #[test]
    fn empty_map_passes_through_all_ids() {
        let map = HashMap::new();
        assert_eq!(
            get_bedrock_model_id(&map, "claude-opus-4-6"),
            "claude-opus-4-6"
        );
    }

    #[test]
    fn anthropic_model_translates_and_preserves_response_model_id() {
        let map = build_anthropic_to_bedrock();
        let incoming_model = "claude-opus-4-6";

        let response_model_id = incoming_model.to_string();
        let bedrock_model_id = get_bedrock_model_id(&map, incoming_model);

        assert_eq!(bedrock_model_id, "us.anthropic.claude-opus-4-6-v1");
        assert_eq!(response_model_id, "claude-opus-4-6");
    }

    #[test]
    fn bedrock_model_passes_through_and_preserves_response_model_id() {
        let map = build_anthropic_to_bedrock();
        let incoming_model = "us.anthropic.claude-haiku-4-5-20251001-v1:0";

        let response_model_id = incoming_model.to_string();
        let bedrock_model_id = get_bedrock_model_id(&map, incoming_model);

        assert_eq!(
            bedrock_model_id,
            "us.anthropic.claude-haiku-4-5-20251001-v1:0"
        );
        assert_eq!(
            response_model_id,
            "us.anthropic.claude-haiku-4-5-20251001-v1:0"
        );
    }
}
