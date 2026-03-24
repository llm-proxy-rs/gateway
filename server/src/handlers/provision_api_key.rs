use anyhow::anyhow;
use apikeys::{create_api_key, get_active_api_key};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, decode_header};
use jwks::{Jwks, jwk_to_decoding_key};
use myhandlers::AppState;
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};
use users::create_user;
use validation::ValidationBuilder;

#[derive(Serialize)]
struct ApiKeyResponse {
    api_key: String,
}

#[derive(Deserialize)]
struct CognitoClaims {
    email: Option<String>,
}

/// POST /api/v1/api-key
///
/// Accepts `Authorization: Bearer <cognito_access_token>`.
/// Validates the JWT against gateway Cognito JWKS, extracts the user email,
/// creates the user if needed, and returns an existing active API key or
/// creates a new one.
pub async fn provision_api_key(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Response, Response> {
    let token = extract_bearer_token(&headers).map_err(|_| {
        warn!("bad Authorization header");
        (
            StatusCode::UNAUTHORIZED,
            "Missing or invalid Authorization header",
        )
            .into_response()
    })?;

    let email = validate_jwt_and_extract_email(&token, &state)
        .await
        .map_err(|_| {
            warn!("JWT validation failed");
            (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response()
        })?;

    info!("provisioning API key for user");

    // Create user if not exists (ignore unique constraint violation)
    if let Err(e) = create_user(&state.db_pool, &email).await {
        let is_duplicate = e
            .downcast_ref::<sqlx::Error>()
            .and_then(|se| se.as_database_error())
            .is_some_and(|de| de.is_unique_violation());
        if !is_duplicate {
            error!("create_user failed");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response());
        }
    }

    // Return existing active key if available
    match get_active_api_key(&state.db_pool, &email).await {
        Ok(Some(key)) => {
            return Ok(Json(ApiKeyResponse { api_key: key }).into_response());
        }
        Ok(None) => {}
        Err(_) => {
            error!("get_active_api_key failed");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response());
        }
    }

    // Create new key
    let api_key = create_api_key(&state.db_pool, &email).await.map_err(|_| {
        error!("create_api_key failed");
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
    })?;

    Ok(Json(ApiKeyResponse {
        api_key: api_key.to_string(),
    })
    .into_response())
}

fn extract_bearer_token(headers: &HeaderMap) -> anyhow::Result<String> {
    let value = headers
        .get("Authorization")
        .ok_or_else(|| anyhow!("missing Authorization header"))?;
    let s = value
        .to_str()
        .map_err(|_| anyhow!("Authorization header contains invalid characters"))?;
    let token = s
        .strip_prefix("Bearer ")
        .ok_or_else(|| anyhow!("Authorization header is not Bearer scheme"))?;
    Ok(token.trim().to_string())
}

async fn validate_jwt_and_extract_email(token: &str, state: &AppState) -> anyhow::Result<String> {
    // Decode header to get kid
    let header = decode_header(token)?;
    let kid = header.kid.ok_or_else(|| anyhow!("JWT missing kid"))?;

    // Fetch JWKS
    let jwks = Jwks::builder()
        .region(&state.cognito_region)
        .user_pool_id(&state.cognito_user_pool_id)
        .build()
        .await?;

    let jwk = jwks
        .find_jwk(&kid)
        .ok_or_else(|| anyhow!("No matching key found in JWKS"))?;

    let decoding_key = jwk_to_decoding_key(&jwk)?;

    // Cognito access tokens don't have aud claim, so skip client_id
    let validation = ValidationBuilder::new()
        .client_id(&state.cognito_client_id)
        .region(&state.cognito_region)
        .user_pool_id(&state.cognito_user_pool_id)
        .build()?;

    let token_data = decode::<CognitoClaims>(token, &decoding_key, &validation)?;

    token_data
        .claims
        .email
        .filter(|e| !e.is_empty())
        .ok_or_else(|| anyhow!("No email found in token claims"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bearer_token_valid() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer my-token-123".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers).unwrap(), "my-token-123");
    }

    #[test]
    fn extract_bearer_token_with_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Bearer  my-token ".parse().unwrap());
        assert_eq!(extract_bearer_token(&headers).unwrap(), "my-token");
    }

    #[test]
    fn extract_bearer_token_missing_header() {
        let headers = HeaderMap::new();
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn extract_bearer_token_wrong_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "Basic abc123".parse().unwrap());
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn extract_bearer_token_no_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", "just-a-token".parse().unwrap());
        assert!(extract_bearer_token(&headers).is_err());
    }

    #[test]
    fn api_key_response_serializes_correctly() {
        let resp = ApiKeyResponse {
            api_key: "test-key-uuid".to_string(),
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["api_key"], "test-key-uuid");
    }
}
