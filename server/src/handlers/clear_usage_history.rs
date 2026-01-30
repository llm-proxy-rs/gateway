use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;
use usage::delete_usage_records;

use crate::csrf::{get_authenticity_token, verify_authenticity_token};
use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct ClearUsageHistoryForm {
    pub authenticity_token: String,
}

pub async fn clear_usage_history_get(
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
                <h1>Clear Usage History</h1>
                <p>Click the button below to delete all your usage history records.</p>
                <p>Warning: This action cannot be undone.</p>
                <form action="/clear-usage-history" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <button type="submit">Clear Usage History</button>
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

pub async fn clear_usage_history_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<ClearUsageHistoryForm>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    let deleted_count = delete_usage_records(state.db_pool.as_ref(), &email).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Usage History Cleared</h1>
                <p>{} usage record(s) deleted.</p>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        deleted_count,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}
