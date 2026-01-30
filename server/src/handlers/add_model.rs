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
pub struct AddModelForm {
    pub authenticity_token: String,
    pub model_name: String,
}

pub async fn add_model_get(token: CsrfToken, session: Session) -> Result<Response, AppError> {
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
                <h1>Add Model</h1>
                <form action="/add-model" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <label for="model_name">Model Name:</label><br>
                    <input type="text" id="model_name" name="model_name" required><br><br>
                    <button type="submit">Add Model</button>
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

pub async fn add_model_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<AddModelForm>,
) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    match models::create_model(&state.db_pool, &form.model_name).await {
        Ok(_) => {
            let html = format!(
                r#"
                <!DOCTYPE html>
                <html>
                <head>
                    {}
                </head>
                <body>
                    <div>
                        <h1>Model Added</h1>
                        <p>Model "{}" has been added successfully.</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
                common_styles(),
                form.model_name,
                nav_menu()
            );
            Ok(Html(html).into_response())
        }
        Err(e) => {
            let error_message = if e.to_string().contains("duplicate key")
                || e.to_string().contains("unique constraint")
            {
                format!("Model \"{}\" already exists.", form.model_name)
            } else {
                format!("Failed to add model: {}", e)
            };

            let html = format!(
                r#"
                <!DOCTYPE html>
                <html>
                <head>
                    {}
                </head>
                <body>
                    <div>
                        <h1>Error</h1>
                        <p style="color: red;">{}</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
                common_styles(),
                error_message,
                nav_menu()
            );
            Ok(Html(html).into_response())
        }
    }
}
