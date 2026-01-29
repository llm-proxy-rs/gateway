use anthropic_request::V1MessagesRequest;
use anyhow::Context;
use apikeys::get_api_key;
use aws_sdk_bedrockruntime::types::TokenUsage;
use axum::{
    Json, Router,
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response, sse::Sse},
    routing::{get, post},
};
use axum_csrf::{CsrfConfig, CsrfLayer, CsrfToken, Key};
use chat::bedrock::ReasoningEffortToThinkingBudgetTokens;
use chat::provider::{
    BedrockChatCompletionsProvider, BedrockV1MessagesProvider, ChatCompletionsProvider,
    V1MessagesProvider,
};
use config::{Config, Environment, File};
use dotenv::dotenv;
use futures::Stream;
use http::header::{AUTHORIZATION, CONTENT_TYPE};
use models::{delete_model, get_models, to_models_response};
use myerrors::AppError;
use myhandlers::{AppState, callback, login, logout};
use request::ChatCompletionsRequest;
use serde::Deserialize;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tokio::signal;
use tokio::task::AbortHandle;
use tower_http::cors::{Any, CorsLayer};
use tower_sessions::{ExpiredDeletion, Expiry, Session, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing::{debug, error, info};
use usage::{CreateUsageRequest, create_usage, get_usage_records};
use users::create_user;

#[derive(Clone, Deserialize)]
struct AppConfig {
    cognito_client_id: String,
    cognito_client_secret: String,
    cognito_domain: String,
    cognito_redirect_uri: String,
    cognito_region: String,
    cognito_user_pool_id: String,
    csrf_cookie_key: String,
    csrf_salt: String,
    #[serde(default = "default_database_url")]
    database_url: String,
    #[serde(default = "default_host")]
    host: String,
    #[serde(default = "default_port")]
    port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

fn default_database_url() -> String {
    "postgres://postgres:postgres@localhost/gateway".to_string()
}

fn nav_menu() -> &'static str {
    r#"<br>
        <a href="/">Home</a>
        <a href="/generate-api-key">Generate API Key</a>
        <a href="/disable-api-keys">Disable API Keys</a>
        <a href="/view-usage-history">View Usage History</a>
        <a href="/update-usage-recording">Update Usage Recording</a>
        <a href="/clear-usage-history">Clear Usage History</a>
        <a href="/browse-models">Browse Models</a>
        <a href="/add-model">Add Model</a>
        <a href="/logout">Logout</a>
    "#
}

fn make_usage_tracker(
    db_pool: Arc<PgPool>,
    api_key: String,
    model_name: String,
) -> impl Fn(&TokenUsage) + Send + Sync + 'static {
    move |usage: &TokenUsage| {
        info!("Usage: {:?}", usage);

        let db_pool = db_pool.clone();

        let create_usage_request = CreateUsageRequest {
            api_key: api_key.clone(),
            model_name: model_name.clone(),
            total_tokens: usage.total_tokens,
        };

        tokio::spawn(async move {
            if let Err(e) = create_usage(&db_pool, create_usage_request).await {
                error!("Failed to create usage: {}", e);
            }
        });
    }
}

async fn chat_completions(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(mut payload): Json<ChatCompletionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    debug!(
        "Received chat completions request for model: {}",
        payload.model
    );

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key in Authorization header")?;

    let (api_key_exists, model_name_exists) =
        select_exists_api_key_and_model_name(&state.db_pool, &api_key, &payload.model).await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    if !model_name_exists {
        error!("Model name validation failed: Invalid model name");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing model name"
        )));
    }

    if payload.stream != Some(true) {
        error!("Streaming is required but was disabled");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    payload.model = payload.model.to_lowercase();

    let usage_callback = make_usage_tracker(
        state.db_pool.clone(),
        api_key.clone(),
        payload.model.clone(),
    );

    let reasoning_effort_to_thinking_budget_tokens =
        ReasoningEffortToThinkingBudgetTokens::default();

    let stream = BedrockChatCompletionsProvider::new()
        .await
        .chat_completions_stream(
            payload,
            reasoning_effort_to_thinking_budget_tokens,
            usage_callback,
        )
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}

async fn v1_messages(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(mut payload): Json<V1MessagesRequest>,
) -> Result<
    (
        StatusCode,
        Sse<impl Stream<Item = Result<axum::response::sse::Event, anyhow::Error>>>,
    ),
    AppError,
> {
    debug!("Received v1/messages request for model: {}", payload.model);

    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key in Authorization header")?;

    let (api_key_exists, model_name_exists) =
        select_exists_api_key_and_model_name(&state.db_pool, &api_key, &payload.model).await?;

    if !api_key_exists {
        error!("API key validation failed: Invalid API key");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    if !model_name_exists {
        error!("Model name validation failed: Invalid model name");
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing model name"
        )));
    }

    if payload.stream != Some(true) {
        error!("Streaming is required but was disabled");
        return Err(AppError::from(anyhow::anyhow!(
            "Streaming is required but was disabled"
        )));
    }

    payload.model = payload.model.to_lowercase();

    let usage_callback = make_usage_tracker(
        state.db_pool.clone(),
        api_key.clone(),
        payload.model.clone(),
    );

    let stream = BedrockV1MessagesProvider::new()
        .await
        .v1_messages_stream(payload, usage_callback)
        .await?;

    Ok((StatusCode::OK, Sse::new(stream)))
}

async fn index(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let email = session.get::<String>("email").await?;

    let html = match email {
        Some(ref email) => {
            let stats = users::get_usage_stats(&state.db_pool, email)
                .await
                .unwrap_or(users::UsageStats {
                    usage_count: 0,
                    total_tokens: 0,
                });

            let (total_keys, active_keys) = sqlx::query!(
                r#"
                SELECT
                    COUNT(*) as "total!",
                    COUNT(*) FILTER (WHERE is_disabled = false) as "active!"
                FROM api_keys
                WHERE user_id = (SELECT user_id FROM users WHERE email = $1)
                "#,
                email
            )
            .fetch_one(state.db_pool.as_ref())
            .await
            .map(|row| (row.total, row.active))
            .unwrap_or((0, 0));

            format!(
                r#"
                <!DOCTYPE html>
                <html>
                <head>
                    <style>
                        table {{
                            border-collapse: collapse;
                            margin: 20px 0 0 0;
                        }}
                        th, td {{
                            border: 1px solid #ddd;
                            padding: 8px;
                            text-align: left;
                        }}
                        th {{
                            background-color: #f2f2f2;
                        }}
                    </style>
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
                stats.usage_count,
                stats.total_tokens,
                active_keys,
                total_keys,
                nav_menu()
            )
        }
        None => r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div>
                    <a href="/login">Login</a>
                </div>
            </body>
            </html>
        "#
        .to_string(),
    };

    if let Some(ref email) = email {
        let _ = create_user(&state.db_pool, email).await;
    }

    Ok(Html(html).into_response())
}

#[derive(Deserialize)]
struct ApiKeyForm {
    authenticity_token: String,
}

async fn verify_authenticity_token(
    token: &CsrfToken,
    session: &Session,
    form_token: &str,
) -> Result<(), AppError> {
    let stored_token: String = match session.get("authenticity_token").await? {
        Some(token) => token,
        None => {
            return Err(AppError::from(anyhow::anyhow!(
                "CSRF token not found in session"
            )));
        }
    };

    if token.verify(form_token).is_err() {
        return Err(AppError::from(anyhow::anyhow!("Invalid CSRF token")));
    }

    if token.verify(&stored_token).is_err() {
        return Err(AppError::from(anyhow::anyhow!(
            "Token mismatch or replay attack detected"
        )));
    }

    session.remove::<String>("authenticity_token").await?;
    Ok(())
}

async fn get_authenticity_token(token: &CsrfToken, session: &Session) -> Result<String, AppError> {
    let authenticity_token = token
        .authenticity_token()
        .map_err(|e| AppError::from(anyhow::anyhow!("Failed to generate CSRF token: {}", e)))?;

    session
        .insert("authenticity_token", &authenticity_token)
        .await?;

    Ok(authenticity_token)
}

async fn generate_api_key_get(token: CsrfToken, session: Session) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
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
        authenticity_token,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

async fn generate_api_key_post(
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

    let api_key = apikeys::create_api_key(&state.db_pool, &email).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
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
        api_key,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

async fn view_usage_history(
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
            <style>
                table {{
                    border-collapse: collapse;
                    width: 100%;
                    margin: 20px 0 0 0;
                }}
                th, td {{
                    border: 1px solid #ddd;
                    padding: 8px;
                }}
                th {{
                    background-color: #f2f2f2;
                }}
            </style>
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
        nav_menu()
    );

    Ok(Html(html).into_response())
}

async fn disable_api_keys_get(token: CsrfToken, session: Session) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
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
        authenticity_token,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

#[derive(Deserialize)]
struct DisableApiKeysForm {
    authenticity_token: String,
}

async fn disable_api_keys_post(
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

    let deleted_count = apikeys::disable_all_api_keys(&state.db_pool, &email).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <body>
            <h1>API Keys Disabled</h1>
            <p>{} API key(s) disabled.</p>
            <a href="/">Back to Home</a>
        </body>
        </html>
        "#,
        deleted_count
    );

    Ok((token, Html(html)).into_response())
}

async fn update_usage_recording_get(
    token: CsrfToken,
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let usage_record =
        sqlx::query_scalar!("SELECT usage_record FROM users WHERE email = $1", email)
            .fetch_one(state.db_pool.as_ref())
            .await?;

    let status = if usage_record { "enabled" } else { "disabled" };
    let action = if usage_record { "Disable" } else { "Enable" };

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
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
        status,
        authenticity_token,
        action,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

#[derive(Deserialize)]
struct UsageRecordingForm {
    authenticity_token: String,
}

async fn update_usage_recording_post(
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

    sqlx::query!(
        "UPDATE users SET usage_record = NOT usage_record WHERE email = $1",
        email
    )
    .execute(state.db_pool.as_ref())
    .await?;

    Ok(Redirect::to("/update-usage-recording").into_response())
}

async fn clear_usage_history_get(token: CsrfToken, session: Session) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
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
        authenticity_token,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

#[derive(Deserialize)]
struct ClearUsageHistoryForm {
    authenticity_token: String,
}

async fn clear_usage_history_post(
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

    let result = sqlx::query!(
        r#"
        DELETE FROM usage
        WHERE user_id = (SELECT user_id FROM users WHERE email = $1)
        "#,
        email
    )
    .execute(state.db_pool.as_ref())
    .await?;

    let deleted_count = result.rows_affected();

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div>
                <h1>Usage History Cleared</h1>
                <p>{} usage record(s) deleted.</p>
                {}
            </div>
        </body>
        </html>
        "#,
        deleted_count,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

async fn browse_models(
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
                    <form action="/delete-model" method="post" style="margin: 0;">
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
            <style>
                table {{
                    border-collapse: collapse;
                    width: 100%;
                    margin: 20px 0 0 0;
                }}
                th, td {{
                    border: 1px solid #ddd;
                    padding: 8px;
                }}
                th {{
                    background-color: #f2f2f2;
                }}
            </style>
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
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

async fn add_model_get(token: CsrfToken, session: Session) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let authenticity_token = get_authenticity_token(&token, &session).await?;

    let html = format!(
        r#"
        <!DOCTYPE html>
        <html>
        <body>
            <div>
                <h1>Add Model</h1>
                <form action="/add-model" method="post">
                    <input type="hidden" name="authenticity_token" value="{}">
                    <label for="model_name">Model Name:</label><br>
                    <input type="text" id="model_name" name="model_name" required style="width: 400px;"><br><br>
                    <button type="submit">Add Model</button>
                </form>
                {}
            </div>
        </body>
        </html>
        "#,
        authenticity_token,
        nav_menu()
    );

    Ok((token, Html(html)).into_response())
}

#[derive(Deserialize)]
struct AddModelForm {
    authenticity_token: String,
    model_name: String,
}

async fn add_model_post(
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
                <body>
                    <div>
                        <h1>Model Added</h1>
                        <p>Model "{}" has been added successfully.</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
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
                <body>
                    <div>
                        <h1>Error</h1>
                        <p style="color: red;">{}</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
                error_message,
                nav_menu()
            );
            Ok(Html(html).into_response())
        }
    }
}

#[derive(Deserialize)]
struct DeleteModelForm {
    authenticity_token: String,
    model_name: String,
}

async fn delete_model_post(
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
                <body>
                    <div>
                        <h1>Model Deleted</h1>
                        <p>Model "{}" has been deleted successfully.</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
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
                <body>
                    <div>
                        <h1>Error</h1>
                        <p style="color: red;">{}</p>
                        {}
                    </div>
                </body>
                </html>
                "#,
                error_message,
                nav_menu()
            );
            Ok(Html(html).into_response())
        }
    }
}

async fn models(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let api_key = get_api_key(&headers)
        .await
        .context("Missing API key in Authorization header")?;

    let api_key_exists = select_exists_api_key(&state.db_pool, &api_key).await?;

    if !api_key_exists {
        return Err(AppError::from(anyhow::anyhow!(
            "Invalid or missing API key"
        )));
    }

    let models = get_models(&state.db_pool).await?;

    let models_response = to_models_response(&models);

    Ok(Json(models_response).into_response())
}

async fn load_config() -> anyhow::Result<AppConfig> {
    let app_config: AppConfig = Config::builder()
        .add_source(File::with_name("config").required(false))
        .add_source(Environment::default())
        .build()?
        .try_deserialize()?;

    Ok(app_config)
}

async fn setup_database(database_url: &str) -> anyhow::Result<PgPool> {
    info!("Connecting to database");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    info!("Database connection established");

    Ok(pool)
}

async fn select_exists_api_key_and_model_name(
    db_pool: &PgPool,
    api_key: &str,
    model_name: &str,
) -> anyhow::Result<(bool, bool)> {
    let result: (bool, bool) = sqlx::query_as(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE),
            EXISTS (SELECT 1 FROM models WHERE model_name = $2);
        "#,
    )
    .bind(api_key)
    .bind(model_name)
    .fetch_one(db_pool)
    .await?;

    Ok(result)
}

async fn select_exists_api_key(db_pool: &PgPool, api_key: &str) -> anyhow::Result<bool> {
    let result: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1 AND is_disabled = FALSE)
        "#,
    )
    .bind(api_key)
    .fetch_one(db_pool)
    .await?;

    Ok(result)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    info!("Initializing LLM proxy server");

    let app_config = load_config().await?;
    info!("Starting server on {}:{}", app_config.host, app_config.port);

    let db_pool = setup_database(&app_config.database_url).await?;
    info!("Database connection pool established");

    if app_config.cognito_client_id.is_empty()
        || app_config.cognito_client_secret.is_empty()
        || app_config.cognito_domain.is_empty()
    {
        error!(
            "Missing required Cognito configuration. Please check your config file or environment variables."
        );
    } else {
        info!("Cognito configuration loaded successfully");
    }

    let app_state = AppState {
        cognito_client_id: app_config.cognito_client_id,
        cognito_client_secret: app_config.cognito_client_secret,
        cognito_domain: app_config.cognito_domain,
        cognito_redirect_uri: app_config.cognito_redirect_uri,
        cognito_region: app_config.cognito_region,
        cognito_user_pool_id: app_config.cognito_user_pool_id,
        db_pool: Arc::new(db_pool.clone()),
    };

    let session_store = PostgresStore::new(db_pool);
    session_store.migrate().await?;

    let deletion_task = tokio::task::spawn(
        session_store
            .clone()
            .continuously_delete_expired(tokio::time::Duration::from_secs(3600)),
    );

    let session_layer = SessionManagerLayer::new(session_store)
        .with_expiry(Expiry::OnInactivity(time::Duration::seconds(86400)))
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let cors_layer = CorsLayer::new()
        .allow_headers([AUTHORIZATION, CONTENT_TYPE])
        .allow_origin(Any);

    let api = Router::new()
        .route("/chat/completions", post(chat_completions))
        .route("/v1/messages", post(v1_messages))
        .route("/models", get(models))
        .layer(cors_layer);

    let mut csrf_config = CsrfConfig::default().with_salt(app_config.csrf_salt);

    let key_bytes: Result<Vec<u8>, _> = app_config
        .csrf_cookie_key
        .split(", ")
        .map(|s| s.trim().parse::<u8>())
        .collect();

    if let Ok(bytes) = key_bytes {
        if bytes.len() == 64 {
            csrf_config = csrf_config.with_key(Some(Key::from(&bytes)));
        } else {
            error!(
                "CSRF cookie key must be exactly 64 bytes, got {}",
                bytes.len()
            );
        }
    } else {
        error!("Failed to parse CSRF cookie key from config");
    }

    let app = Router::new()
        .route("/", get(index))
        .route("/add-model", get(add_model_get).post(add_model_post))
        .route("/browse-models", get(browse_models))
        .route("/callback", get(callback))
        .route("/delete-model", post(delete_model_post))
        .route(
            "/disable-api-keys",
            get(disable_api_keys_get).post(disable_api_keys_post),
        )
        .route(
            "/generate-api-key",
            get(generate_api_key_get).post(generate_api_key_post),
        )
        .route("/login", get(login))
        .route("/logout", get(logout))
        .route("/view-usage-history", get(view_usage_history))
        .route(
            "/update-usage-recording",
            get(update_usage_recording_get).post(update_usage_recording_post),
        )
        .route(
            "/clear-usage-history",
            get(clear_usage_history_get).post(clear_usage_history_post),
        )
        .merge(api)
        .layer(CsrfLayer::new(csrf_config))
        .layer(session_layer)
        .with_state(app_state);

    info!(
        "Routes configured, binding to {}:{}",
        app_config.host, app_config.port
    );
    let listener =
        tokio::net::TcpListener::bind(format!("{}:{}", app_config.host, app_config.port)).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(deletion_task.abort_handle()))
        .await?;

    deletion_task.await??;

    Ok(())
}

async fn shutdown_signal(deletion_task_abort_handle: AbortHandle) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { deletion_task_abort_handle.abort() },
        _ = terminate => { deletion_task_abort_handle.abort() },
    }
}
