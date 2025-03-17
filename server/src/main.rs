use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chat::providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider};
use config::{Config, File};
use dotenv::dotenv;
use handlers::CallbackQuery;
use request::ChatCompletionsRequest;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::sync::Arc;
use tower_sessions::{MemoryStore, Session, SessionManagerLayer, cookie::SameSite};
use tracing::{debug, error, info};
use usage::{CreateUsageRequest, create_usage, get_usage_records};
use users::create_user;
mod error;

use crate::error::AppError;

#[derive(Clone)]
struct AppState {
    db_pool: Arc<PgPool>,
    cognito_client_id: String,
    cognito_client_secret: String,
    cognito_domain: String,
    cognito_redirect_uri: String,
    cognito_region: String,
    cognito_user_pool_id: String,
}

#[derive(Clone)]
struct AppConfig {
    cognito_client_id: String,
    cognito_client_secret: String,
    cognito_domain: String,
    cognito_redirect_uri: String,
    cognito_region: String,
    cognito_user_pool_id: String,
}

async fn chat_completions(
    headers: HeaderMap,
    state: State<AppState>,
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

    let model_name = payload.model.clone();

    if payload.model.starts_with("bedrock/") {
        payload.model = payload.model.replace("bedrock/", "");
    }

    let provider = BedrockChatCompletionsProvider::new().await;
    info!(
        "Processing chat completions request with {} messages",
        payload.messages.len()
    );

    let stream = provider
        .chat_completions_stream(payload, move |usage| {
            info!(
                "Usage: prompt_tokens: {}, completion_tokens: {}, total_tokens: {}",
                usage.prompt_tokens, usage.completion_tokens, usage.total_tokens
            );

            let db_pool = state.db_pool.clone();
            let create_usage_request = CreateUsageRequest {
                api_key: api_key.clone(),
                model_name: model_name.clone(),
                input_tokens: usage.prompt_tokens,
                output_tokens: usage.completion_tokens,
            };

            tokio::spawn(async move {
                if let Err(e) = create_usage(&db_pool, create_usage_request).await {
                    error!("Failed to create usage: {}", e);
                }
            });
        })
        .await?;
    Ok((StatusCode::OK, stream))
}

async fn index(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let email = session.get::<String>("email").await?;

    let html = match email {
        Some(ref email) => {
            let total_spent = users::get_total_spent(&state.db_pool, email)
                .await
                .unwrap_or_default();

            format!(
                r#"
                <!DOCTYPE html>
                <html>
                <body>
                    <div>
                        <h1>Welcome, {email}!</h1>
                        <p>Total spent: ${total_spent}</p>
                        <a href="/logout">Logout</a>
                        <a href="/generate-api-key">Generate API Key</a>
                        <a href="/usage-history">View Usage History</a>
                    </div>
                </body>
                </html>
                "#
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

async fn logout(session: Session) -> Result<Response, AppError> {
    session.delete().await?;
    Ok(Redirect::to("/").into_response())
}

async fn login(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let state = State(handlers::AppState {
        client_id: state.cognito_client_id.clone(),
        client_secret: state.cognito_client_secret.clone(),
        domain: state.cognito_domain.clone(),
        redirect_uri: state.cognito_redirect_uri.clone(),
        region: state.cognito_region.clone(),
        user_pool_id: state.cognito_user_pool_id.clone(),
    });
    Ok(handlers::login(session, state).await?)
}

async fn callback(
    query: Query<CallbackQuery>,
    session: Session,
    state: State<AppState>,
) -> Result<Response, AppError> {
    let state = State(handlers::AppState {
        client_id: state.cognito_client_id.clone(),
        client_secret: state.cognito_client_secret.clone(),
        domain: state.cognito_domain.clone(),
        redirect_uri: state.cognito_redirect_uri.clone(),
        region: state.cognito_region.clone(),
        user_pool_id: state.cognito_user_pool_id.clone(),
    });
    Ok(handlers::callback(query, session, state).await?)
}

async fn generate_api_key(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

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
                <a href="/">Back to Home</a>
            </div>
        </body>
        </html>
        "#,
        api_key
    );

    Ok(Html(html).into_response())
}

async fn usage_history(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let usage_records = get_usage_records(&state.db_pool, &email, 100).await?;

    let mut rows = String::new();
    for record in usage_records {
        let total_tokens = record.total_input_tokens + record.total_output_tokens;

        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>{}</td>
                <td>{}</td>
                <td>${}</td>
                <td>{}</td>
                <td>{}</td>
            </tr>"#,
            record.model_name,
            total_tokens,
            record.created_at,
            record.total_cost(),
            record.total_input_tokens,
            record.total_output_tokens
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
                    margin: 20px 0;
                }}
                th, td {{
                    border: 1px solid #ddd;
                    padding: 8px;
                }}
                th {{
                    background-color: #f2f2f2;
                }}
                tr:nth-child(even) {{
                    background-color: #f9f9f9;
                }}
            </style>
        </head>
        <body>
            <div>
                <h1>Last 100 Usage Records for {email}</h1>
                <table>
                    <thead>
                        <tr>
                            <th>Model</th>
                            <th>Total Tokens</th>
                            <th>Date</th>
                            <th>Cost</th>
                            <th>Input Tokens</th>
                            <th>Output Tokens</th>
                        </tr>
                    </thead>
                    <tbody>
                        {rows}
                    </tbody>
                </table>
                <a href="/">Back to Home</a>
            </div>
        </body>
        </html>
        "#
    );

    Ok(Html(html).into_response())
}

async fn load_config() -> anyhow::Result<(String, u16, String, AppConfig)> {
    let settings = Config::builder()
        .add_source(File::with_name("config"))
        .build()?;

    let host: String = settings
        .get("host")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let port: u16 = settings.get("port").unwrap_or(3000);
    let database_url: String = settings.get("database_url").unwrap_or_else(|_| {
        env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost/gateway".to_string())
    });

    let cognito_config = AppConfig {
        cognito_client_id: settings
            .get("cognito_client_id")
            .unwrap_or_else(|_| env::var("COGNITO_CLIENT_ID").unwrap_or_default()),
        cognito_client_secret: settings
            .get("cognito_client_secret")
            .unwrap_or_else(|_| env::var("COGNITO_CLIENT_SECRET").unwrap_or_default()),
        cognito_region: settings.get("cognito_region").unwrap_or_else(|_| {
            env::var("COGNITO_REGION").unwrap_or_else(|_| "us-east-1".to_string())
        }),
        cognito_user_pool_id: settings
            .get("cognito_user_pool_id")
            .unwrap_or_else(|_| env::var("COGNITO_USER_POOL_ID").unwrap_or_default()),
        cognito_redirect_uri: settings
            .get("cognito_redirect_uri")
            .unwrap_or_else(|_| env::var("COGNITO_REDIRECT_URI").unwrap_or_default()),
        cognito_domain: settings
            .get("cognito_domain")
            .unwrap_or_else(|_| env::var("COGNITO_DOMAIN").unwrap_or_default()),
    };

    Ok((host, port, database_url, cognito_config))
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
            EXISTS (SELECT 1 FROM api_keys WHERE api_key = $1),
            EXISTS (SELECT 1 FROM models WHERE model_name = $2);
        "#,
    )
    .bind(api_key)
    .bind(model_name)
    .fetch_one(db_pool)
    .await?;

    Ok(result)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    info!("Initializing LLM proxy server");

    let (host, port, database_url, cognito_config) = load_config().await?;
    info!("Starting server on {}:{}", host, port);

    let db_pool = Arc::new(setup_database(&database_url).await?);
    info!("Database connection pool established");

    if cognito_config.cognito_client_id.is_empty()
        || cognito_config.cognito_client_secret.is_empty()
        || cognito_config.cognito_domain.is_empty()
    {
        error!(
            "Missing required Cognito configuration. Please check your config file or environment variables."
        );
    } else {
        info!("Cognito configuration loaded successfully");
    }

    let app_state = AppState {
        db_pool,
        cognito_client_id: cognito_config.cognito_client_id,
        cognito_client_secret: cognito_config.cognito_client_secret,
        cognito_domain: cognito_config.cognito_domain,
        cognito_redirect_uri: cognito_config.cognito_redirect_uri,
        cognito_region: cognito_config.cognito_region,
        cognito_user_pool_id: cognito_config.cognito_user_pool_id,
    };

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_same_site(SameSite::Lax)
        .with_secure(false);

    let app = Router::new()
        .route("/", get(index))
        .route("/chat/completions", post(chat_completions))
        .route("/callback", get(callback))
        .route("/login", get(login))
        .route("/logout", get(logout))
        .route("/generate-api-key", get(generate_api_key))
        .route("/usage-history", get(usage_history))
        .layer(session_layer)
        .with_state(app_state);

    info!("Routes configured, binding to {}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
    info!("Server started successfully, listening for requests");

    axum::serve(listener, app).await?;

    Ok(())
}
