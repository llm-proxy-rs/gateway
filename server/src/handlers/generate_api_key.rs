use apikeys::create_api_key;
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
pub struct ApiKeyForm {
    pub authenticity_token: String,
}

pub async fn generate_api_key_get(
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
                <h1>Generate API Key</h1>
                <p>Click the button below to generate a new API key.</p>
                <form action="/generate-api-key" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <button type="submit">Generate API Key</button>
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

pub async fn generate_api_key_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<ApiKeyForm>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    let api_key = create_api_key(&state.db_pool, &email).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Your API Key</h1>
                <p>Please save this key securely. It will not be shown again.</p>
                <pre>{}</pre>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        api_key,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}
