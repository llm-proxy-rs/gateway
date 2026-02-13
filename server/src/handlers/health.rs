use axum::{extract::State, http::StatusCode, response::IntoResponse};
use myhandlers::AppState;

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query("SELECT 1").execute(&*state.db_pool).await {
        Ok(_) => (StatusCode::OK, "ok"),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, "unhealthy"),
    }
}
