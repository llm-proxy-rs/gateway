use apikeys::get_api_keys_count_and_api_keys_count_active;
use axum::{
    extract::State,
    response::{Html, IntoResponse, Response},
};
use myerrors::AppError;
use myhandlers::AppState;
use tower_sessions::Session;
use users::create_user;

use crate::templates::common::{common_styles, nav_menu};

pub async fn index(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let email = session.get::<String>("email").await?;

    let html = match email {
        Some(ref email) => {
            let (user_usage_count, user_total_tokens) =
                users::get_user_usage_count_and_user_total_tokens(&state.db_pool, email)
                    .await
                    .unwrap_or((0, 0));

            let (api_keys_count, api_keys_count_active) =
                get_api_keys_count_and_api_keys_count_active(&state.db_pool, email)
                    .await
                    .unwrap_or((0, 0));

            format!(
                r#"
                <!DOCTYPE html>
                <html>
                <head>
                    {}
                </head>
                <body>
                    <div>
                        <h1>Welcome, {email}!</h1>
                        <table>
                            <tr>
                                <th>Total usage</th>
                                <td>{} requests ({} tokens)</td>
                            </tr>
                            <tr>
                                <th>API keys</th>
                                <td>{} active ({} total)</td>
                            </tr>
                        </table>
                        {}
                    </div>
                </body>
                </html>
                "#,
                common_styles(),
                user_usage_count,
                user_total_tokens,
                api_keys_count_active,
                api_keys_count,
                nav_menu()
            )
        }
        None => format!(
            r#"
            <!DOCTYPE html>
            <html>
            <head>
                {}
            </head>
            <body>
                <div>
                    <a href="/login">Login</a>
                </div>
            </body>
            </html>
            "#,
            common_styles()
        ),
    };

    if let Some(ref email) = email {
        let _ = create_user(&state.db_pool, email).await;
    }

    Ok(Html(html).into_response())
}
