use axum::response::{IntoResponse, Redirect, Response};
use myerrors::AppError;
use tower_sessions::Session;

pub async fn logout(session: Session) -> Result<Response, AppError> {
    session.delete().await?;
    Ok(Redirect::to("/").into_response())
}
