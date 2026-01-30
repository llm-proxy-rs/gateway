use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;

#[derive(Deserialize)]
pub struct Model {
    pub model_name: String,
    pub protected: bool,
}

#[derive(Serialize)]
pub struct ModelResponse {
    pub model_name: String,
    pub protected: bool,
    pub created_at: OffsetDateTime,
}

#[derive(Serialize)]
pub struct Data {
    pub created: i64,
    pub id: String,
    pub object: String,
    pub owned_by: String,
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub data: Vec<Data>,
    pub object: String,
}

pub async fn get_models(pool: &PgPool) -> anyhow::Result<Vec<Model>> {
    let models = sqlx::query_as!(
        Model,
        r#"
        SELECT
            model_name,
            protected
        FROM models
        ORDER BY model_name
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(models)
}

pub async fn create_model(pool: &PgPool, model_name: &str) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO models (model_name)
        VALUES ($1)
        "#,
        model_name.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn delete_model(pool: &PgPool, model_name: &str) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        DELETE FROM models
        WHERE model_name = $1 AND protected = false
        "#,
        model_name.to_lowercase()
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub fn to_models_response(models: &[Model]) -> ModelsResponse {
    let data = models
        .iter()
        .map(|model| Data {
            created: 0,
            id: model.model_name.clone(),
            object: "model".to_string(),
            owned_by: "".to_string(),
        })
        .collect();

    ModelsResponse {
        data,
        object: "list".to_string(),
    }
}
