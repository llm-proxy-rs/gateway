use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use axum_csrf::CsrfToken;
use models::get_models;
use myerrors::AppError;
use myhandlers::AppState;
use tower_sessions::Session;

use crate::csrf::get_authenticity_token;
use crate::templates::common::{common_styles, nav_menu};

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
            let enable_or_disable_button = if model.is_disabled {
                format!(
                    r#"<form action="/enable-model" method="post" style="display:inline">
                        <input type="hidden" name="authenticity_token" value="{}">
                        <input type="hidden" name="model_name" value="{}">
                        <button type="submit">Enable</button>
                    </form>"#,
                    authenticity_token, model.model_name
                )
            } else {
                format!(
                    r#"<form action="/disable-model" method="post" style="display:inline">
                        <input type="hidden" name="authenticity_token" value="{}">
                        <input type="hidden" name="model_name" value="{}">
                        <button type="submit">Disable</button>
                    </form>"#,
                    authenticity_token, model.model_name
                )
            };

            let delete_button = format!(
                r#"<form action="/delete-model" method="post" style="display:inline">
                        <input type="hidden" name="authenticity_token" value="{}">
                        <input type="hidden" name="model_name" value="{}">
                        <button type="submit">Delete</button>
                    </form>"#,
                authenticity_token, model.model_name
            );

            format!(
                r#"<td>
                    {}
                    {}
                </td>"#,
                enable_or_disable_button, delete_button
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
