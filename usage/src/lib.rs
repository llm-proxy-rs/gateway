use anyhow::Result;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use serde::Serialize;
use sqlx::PgPool;
use sqlx::types::time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

#[serde_with::serde_as]
#[derive(Serialize)]
pub struct Usage {
    pub model_name: String,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_input_cost: Option<f64>,
    pub total_output_cost: Option<f64>,
    #[serde_as(as = "Rfc3339")]
    pub created_at: OffsetDateTime,
}

impl Usage {
    pub fn total_cost(&self) -> Decimal {
        let total_input_cost = self.total_input_cost.unwrap_or_default();
        let total_output_cost = self.total_output_cost.unwrap_or_default();
        let total_input_cost = Decimal::from_f64(total_input_cost).unwrap_or_default();
        let total_output_cost = Decimal::from_f64(total_output_cost).unwrap_or_default();
        total_input_cost + total_output_cost
    }
}

pub struct CreateUsageRequest {
    pub api_key: String,
    pub model_name: String,
    pub input_tokens: i32,
    pub output_tokens: i32,
}

pub async fn create_usage(pool: &PgPool, create_usage: CreateUsageRequest) -> Result<()> {
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"
        WITH usage_insert AS (
            INSERT INTO usage (
                api_key_id, model_id, user_id, 
                total_input_cost, total_input_tokens,
                total_output_cost, total_output_tokens
            )
            SELECT 
                ak.api_key_id, m.model_id, ak.user_id,
                (m.input_price_per_token * $3)::numeric, $3,
                (m.output_price_per_token * $4)::numeric, $4
            FROM 
                (
                    SELECT api_key_id, user_id 
                    FROM api_keys 
                    WHERE api_key = $1
                ) ak,
                (
                    SELECT model_id, input_price_per_token, output_price_per_token 
                    FROM models 
                    WHERE model_name = $2
                ) m
            RETURNING 
                user_id,
                (total_input_cost + total_output_cost)::numeric AS total_cost,
                (total_input_tokens + total_output_tokens) AS total_tokens
        )
        UPDATE users u
        SET
            total_spent = CASE
                WHEN date_trunc('month', u.updated_at) = date_trunc('month', now())
                THEN (u.total_spent + ui.total_cost)::numeric
                ELSE ui.total_cost::numeric
            END,
            total_tokens = CASE
                WHEN date_trunc('month', u.updated_at) = date_trunc('month', now())
                THEN u.total_tokens + ui.total_tokens
                ELSE ui.total_tokens
            END,
            updated_at = now()
        FROM usage_insert ui
        WHERE u.user_id = ui.user_id
        "#,
        create_usage.api_key,
        create_usage.model_name,
        create_usage.input_tokens as i64,
        create_usage.output_tokens as i64
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

pub async fn get_usage_records(pool: &PgPool, email: &str, limit: i64) -> Result<Vec<Usage>> {
    let records = sqlx::query_as!(
        Usage,
        r#"
        SELECT 
            m.model_name,
            u.total_input_tokens,
            u.total_output_tokens,
            u.total_input_cost::float8 AS total_input_cost,
            u.total_output_cost::float8 AS total_output_cost,
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
