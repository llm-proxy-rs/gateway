use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use handlers::CallbackQuery;
use myerrors::AppError;
use sqlx::PgPool;
use std::sync::Arc;
use tower_sessions::Session;

#[derive(Clone)]
pub struct AppState {
    pub cognito_client_id: String,
    pub cognito_client_secret: String,
    pub cognito_domain: String,
    pub cognito_redirect_uri: String,
    pub cognito_region: String,
    pub cognito_user_pool_id: String,
    pub db_pool: Arc<PgPool>,
    pub openai_api_key: Option<String>,
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
