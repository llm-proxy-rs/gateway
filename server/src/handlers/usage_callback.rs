use aws_sdk_bedrockruntime::types::TokenUsage;
use tracing::info;

pub fn create_usage_callback(model_name: &str) -> impl Fn(&TokenUsage) + Send + Sync + 'static {
    let model_name = model_name.to_string();
    move |token_usage: &TokenUsage| {
        info!("Usage for model {}: {:?}", model_name, token_usage);
    }
}
