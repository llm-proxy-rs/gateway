use axum::{
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
};
use myerrors::AppError;
use myhandlers::AppState;
use tower_sessions::Session;
use usage::get_usage_records;

use crate::templates::common::{common_styles, nav_menu};

pub async fn view_usage_history(
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let usage_records = get_usage_records(&state.db_pool, &email, 100).await?;

    let mut rows = String::new();
    for record in usage_records {
        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            record.model_name, record.total_tokens, record.created_at
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
                <h1>View Last 100 Usage Records for {email}</h1>
                <table>
                    <thead>
                        <tr>
                            <th>Model</th>
                            <th>Total Tokens</th>
                            <th>Date</th>
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

    Ok(Html(html).into_response())
}
