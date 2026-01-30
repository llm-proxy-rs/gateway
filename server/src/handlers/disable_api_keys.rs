use apikeys::disable_all_api_keys;
use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;

use crate::csrf::{get_authenticity_token, verify_authenticity_token};
use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct DisableApiKeysForm {
    pub authenticity_token: String,
}

pub async fn disable_api_keys_get(
    token: CsrfToken,
    session: Session,
) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Disable API Keys</h1>
                <p>Click the button below to disable all your API keys.</p>
                <p>Warning: This action cannot be undone.</p>
                <form action="/disable-api-keys" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <button type="submit">Disable API Keys</button>
                </form>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        authenticity_token,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

pub async fn disable_api_keys_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    Form(form): Form<DisableApiKeysForm>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    let disabled_api_keys_count = disable_all_api_keys(&state.db_pool, &email).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <h1>API Keys Disabled</h1>
            <p>{} API key(s) disabled.</p>
            <a href="/">Back to Home</a>
        </body>
        </html>
        "#,
        common_styles(),
        disabled_api_keys_count
    );

    Ok((token, Html(html)).into_response())
}
