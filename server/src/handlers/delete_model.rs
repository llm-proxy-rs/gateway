use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use models::delete_model;
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;

use crate::csrf::verify_authenticity_token;
use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct DeleteModelForm {
    pub authenticity_token: String,
    pub model_name: String,
}

pub async fn delete_model_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<DeleteModelForm>,
) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    delete_model(&state.db_pool, &form.model_name).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Model Deleted</h1>
                <p>Model "{}" has been deleted successfully.</p>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        form.model_name,
        nav_menu()
    );
    Ok((token, Html(html)).into_response())
}
