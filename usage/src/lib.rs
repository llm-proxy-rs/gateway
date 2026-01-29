use anyhow::Result;
use serde::Serialize;
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[serde_with::serde_as]
#[derive(Serialize)]
pub struct Usage {
    pub model_name: String,
    pub total_tokens: i64,
    #[serde_as(as = "Rfc3339")]
    pub created_at: OffsetDateTime,
}

pub struct CreateUsageRequest {
    pub api_key: String,
    pub model_name: String,
    pub total_tokens: i32,
}

pub async fn create_usage(pool: &PgPool, create_usage: CreateUsageRequest) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO usage (
            api_key_id, model_id, user_id,
            total_tokens
        )
        SELECT
            ak.api_key_id, m.model_id, ak.user_id,
            $3
        FROM
            (
                SELECT api_key_id, user_id
                FROM api_keys
                WHERE api_key = $1
            ) ak
        JOIN
            (
                SELECT model_id
                FROM models
                WHERE model_name = $2
            ) m ON true
        JOIN
            users u ON u.user_id = ak.user_id AND u.usage_record = true
        "#,
        create_usage.api_key,
        create_usage.model_name,
        create_usage.total_tokens as i64
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_usage_records(pool: &PgPool, email: &str, limit: i64) -> Result<Vec<Usage>> {
    let records = sqlx::query_as!(
        Usage,
        r#"
        SELECT
            m.model_name,
            u.total_tokens,
            u.created_at
        FROM
            usage u
        JOIN
            models m ON u.model_id = m.model_id
        JOIN
            users usr ON u.user_id = usr.user_id
        WHERE
            usr.email = $1
            AND date_trunc('month', u.created_at) = date_trunc('month', now())
        ORDER BY
            u.created_at DESC
        LIMIT $2
        "#,
        email,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(records)
}
