mod config;
mod csrf;
mod database;
mod handlers;
mod templates;
mod validation;

use axum::{
    Router,
    routing::{get, post},
};
use axum_csrf::{CsrfConfig, CsrfLayer, Key};
use dotenv::dotenv;
use http::header::{AUTHORIZATION, CONTENT_TYPE};
use myhandlers::{AppState, callback, login, logout};
use std::sync::Arc;
use tokio::signal;
use tokio::task::AbortHandle;
use tower_http::cors::{Any, CorsLayer};
use tower_sessions::{ExpiredDeletion, Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;
use tracing::{error, info};

use config::load_config;
use database::setup_database;
use handlers::{
    add_model::{add_model_get, add_model_post},
    browse_models::{browse_models_get, browse_models_post},
    chat_completions::chat_completions,
    clear_usage_history::{clear_usage_history_get, clear_usage_history_post},
    disable_api_keys::{disable_api_keys_get, disable_api_keys_post},
    generate_api_key::{generate_api_key_get, generate_api_key_post},
    index::index,
    models::models,
    update_usage_recording::{update_usage_recording_get, update_usage_recording_post},
    v1_messages::v1_messages,
    view_usage_history::view_usage_history,
};

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
        .route(
            "/browse-models",
            get(browse_models_get).post(browse_models_post),
        )
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
