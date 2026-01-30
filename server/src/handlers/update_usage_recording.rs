use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;
use users::{get_user_usage_tracking_enabled, toggle_user_usage_tracking};

use crate::csrf::{get_authenticity_token, verify_authenticity_token};
use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct UsageRecordingForm {
    pub authenticity_token: String,
}

pub async fn update_usage_recording_get(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let user_usage_tracking_enabled =
        get_user_usage_tracking_enabled(state.db_pool.as_ref(), &email).await?;

    let status = if user_usage_tracking_enabled {
        "enabled"
    } else {
        "disabled"
    };
    let action = if user_usage_tracking_enabled {
        "Disable"
    } else {
        "Enable"
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
                <h1>Update Usage Recording</h1>
                <p>Usage recording is currently <strong>{}</strong>.</p>
                <form action="/update-usage-recording" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <button type="submit">{} Usage Recording</button>
                </form>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        status,
        authenticity_token,
        action,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

pub async fn update_usage_recording_post(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
    form: Form<UsageRecordingForm>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    verify_authenticity_token(&token, &session, &form.authenticity_token).await?;

    toggle_user_usage_tracking(state.db_pool.as_ref(), &email).await?;

    Ok(Redirect::to("/update-usage-recording").into_response())
}
