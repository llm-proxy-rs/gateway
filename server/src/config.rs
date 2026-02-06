use config::{Config, Environment, File};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct AppConfig {
    pub cognito_client_id: String,
    pub cognito_client_secret: String,
    pub cognito_domain: String,
    pub cognito_redirect_uri: String,
    pub cognito_region: String,
    pub cognito_user_pool_id: String,
    pub csrf_cookie_key: String,
    pub csrf_salt: String,
    #[serde(default = "default_inference_profile_prefixes")]
    pub inference_profile_prefixes: Vec<String>,
    #[serde(default = "default_database_url")]
    pub database_url: String,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
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

fn default_inference_profile_prefixes() -> Vec<String> {
    vec!["us.".to_string()]
}

pub async fn load_config() -> anyhow::Result<AppConfig> {
    let app_config: AppConfig = Config::builder()
        .add_source(File::with_name("config").required(false))
        .add_source(Environment::default())
        .build()?
        .try_deserialize()?;

    Ok(app_config)
}
