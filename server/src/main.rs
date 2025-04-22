use anyhow::Context;
use apikeys::get_api_key;
use axum::{
    Json, Router,
    extract::{Form, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response, sse::Sse},
    routing::{get, post},
};
use axum_csrf::{CsrfConfig, CsrfLayer, CsrfToken};
use chat::{
    openai::OpenAIChatCompletionsProvider,
    providers::{BedrockChatCompletionsProvider, ChatCompletionsProvider},
};
use config::{Config, Environment, File};
use dotenv::dotenv;
use http::header::{AUTHORIZATION, CONTENT_TYPE};
use models::{get_models, to_models_response};
use myerrors::AppError;
use myhandlers::{AppState, callback, login, logout};
use request::ChatCompletionsRequest;
use response::Usage;
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
    #[serde(default = "default_database_url")]
    database_url: String,
    #[serde(default = "default_host")]
    host: String,
    openai_api_key: Option<String>,
    #[serde(default = "default_port")]
    port: u16,
    csrf_salt: String,
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

    let model_name = payload.model.clone();

    let usage_callback = move |usage: &Usage| {
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
    };

    let stream = if payload.model.starts_with("openai/") {
        payload.model = payload.model.replace("openai/", "");
        info!("Using OpenAI provider for model: {}", payload.model);
        if let Some(openai_api_key) = &state.openai_api_key {
            if openai_api_key.is_empty() {
                error!("OpenAI API key is empty but OpenAI model was requested");
                return Err(AppError::from(anyhow::anyhow!(
                    "OpenAI API key is empty but OpenAI model was requested"
                )));
            }
            OpenAIChatCompletionsProvider::new(openai_api_key)
                .chat_completions_stream(payload, usage_callback)
                .await?
        } else {
            error!("OpenAI API key is not configured but OpenAI model was requested");
            return Err(AppError::from(anyhow::anyhow!(
                "OpenAI API key is not configured but OpenAI model was requested"
            )));
        }
    } else {
        payload.model = payload.model.replace("bedrock/", "");
        info!("Using Bedrock provider for model: {}", payload.model);
        BedrockChatCompletionsProvider::new()
            .await
            .chat_completions_stream(payload, usage_callback)
            .await?
    };

    Ok((StatusCode::OK, Sse::new(stream)))
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
                        <a href="/disable-api-keys">Disable API Keys</a>
                        <a href="/usage-history">View Usage History</a>
                        <a href="/browse-models">Browse Models</a>
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
                <a href="/">Back to Home</a>
            </div>
        </body>
        </html>
        "#,
        authenticity_token
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
                <a href="/">Back to Home</a>
            </div>
        </body>
        </html>
        "#,
        api_key
    );

    Ok((token, Html(html)).into_response())
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
                <a href="/">Back to Home</a>
            </div>
        </body>
        </html>
        "#,
        authenticity_token
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

async fn browse_models(session: Session, state: State<AppState>) -> Result<Response, AppError> {
    let _email = match session.get::<String>("email").await? {
        Some(email) => email,
        None => return Ok(Redirect::to("/login").into_response()),
    };

    let models = get_models(&state.db_pool).await?;

    let mut rows = String::new();
    for model in models {
        rows.push_str(&format!(
            r#"<tr>
                <td>{}</td>
                <td>${}</td>
                <td>${}</td>
            </tr>"#,
            model.model_name, model.input_price_per_token, model.output_price_per_token
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
                <h1>Available Models</h1>
                <table>
                    <thead>
                        <tr>
                            <th>Model</th>
                            <th>Input Price Per Token</th>
                            <th>Output Price Per Token</th>
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

    if app_config.openai_api_key.is_some() {
        info!("OpenAI API key found in configuration");
    } else {
        info!("No OpenAI API key found in configuration, OpenAI models will not be available");
    }

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
        openai_api_key: app_config.openai_api_key,
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
        .route("/models", get(models))
        .layer(cors_layer);

    let app = Router::new()
        .route("/", get(index))
        .route("/browse-models", get(browse_models))
        .route("/callback", get(callback))
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
        .route("/usage-history", get(usage_history))
        .merge(api)
        .layer(CsrfLayer::new(
            CsrfConfig::default().with_salt(app_config.csrf_salt),
        ))
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
