use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use models::{delete_model, get_models};
use myerrors::AppError;
use myhandlers::AppState;
use serde::Deserialize;
use tower_sessions::Session;

use crate::csrf::{get_authenticity_token, verify_authenticity_token};
use crate::templates::common::{common_styles, nav_menu};

#[derive(Deserialize)]
pub struct DeleteModelForm {
    pub authenticity_token: String,
    pub model_name: String,
}

pub async fn browse_models_get(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let models = get_models(&state.db_pool).await?;

    let mut rows = String::new();
    for model in models {
        let action_cell = if model.protected {
            "<td></td>".to_string()
        } else {
            format!(
                r#"<td>
                    <form action="/browse-models" method="post" style="margin: 0;">
                        <input type="hidden" name="authenticity_token" value="{}">
                        <input type="hidden" name="model_name" value="{}">
                        <button type="submit">Delete</button>
                    </form>
                </td>"#,
                authenticity_token, model.model_name
            )
        };

        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                {}
            </tr>"#,
            model.model_name, action_cell
        ));
    }

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            {}
        </head>
        <body>
            <div>
                <h1>Browse Models</h1>
                <table>
                    <thead>
                        <tr>
                            <th>Model</th>
                            <th>Action</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows}
                    </tbody>
                </table>
                {}
            </div>
        </body>
        </html>
        "#,
        common_styles(),
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

pub async fn browse_models_post(
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

    match delete_model(&state.db_pool, &form.model_name).await {
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
            Ok(Html(html).into_response())
        }
        Err(e) => {
            let error_message = if e.to_string().contains("foreign key constraint")
                || e.to_string().contains("violates foreign key")
            {
                format!(
                    "Cannot delete model \"{}\". It is still referenced by usage records.",
                    form.model_name
                )
            } else {
                format!("Failed to delete model: {}", e)
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
