use axum_csrf::CsrfToken;
use myerrors::AppError;
use tower_sessions::Session;

pub async fn verify_authenticity_token(
    token: &CsrfToken,
    session: &Session,
    form_token: &str,
) -> Result<(), AppError> {
    let stored_token: String = match session.get("authenticity_token").await? {
        Some(token) => token,
        None => {
            return Err(AppError::from(anyhow::anyhow!(
                "CSRF token not found in session"
            )));
        }
    };

    if token.verify(form_token).is_err() {
        return Err(AppError::from(anyhow::anyhow!("Invalid CSRF token")));
    }

    if token.verify(&stored_token).is_err() {
        return Err(AppError::from(anyhow::anyhow!(
            "Token mismatch or replay attack detected"
        )));
    }

    session.remove::<String>("authenticity_token").await?;
    Ok(())
}

pub async fn get_authenticity_token(
    token: &CsrfToken,
    session: &Session,
) -> Result<String, AppError> {
    let authenticity_token = token
        .authenticity_token()
        .map_err(|e| AppError::from(anyhow::anyhow!("Failed to generate CSRF token: {}", e)))?;

    session
        .insert("authenticity_token", &authenticity_token)
        .await?;

    Ok(authenticity_token)
}
