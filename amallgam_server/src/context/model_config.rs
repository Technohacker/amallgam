use std::sync::Arc;

use anyhow::Result;
use llama_cpp::LlamaModel;
use minijinja::Environment;
use serde::Deserialize;
use sqlx::{Row, sqlite::SqliteRow};

use super::AmallgamContext;

#[derive(Debug, Clone, Hash, PartialEq, Eq, Deserialize)]
pub struct ModelId(pub String);

/// Model config stored on DB
#[derive(Deserialize)]
pub struct ModelConfig {
    pub id: ModelId,
    pub file_name: String,
    pub prompt_template: String,
}

#[derive(Clone)]
pub struct LoadedModel {
    pub model: LlamaModel,
    pub template_env: Arc<Environment<'static>>,
}

impl AmallgamContext {
    pub async fn upsert_model_config(&self, model_config: ModelConfig) -> Result<()> {
        sqlx::query(
            "
            INSERT INTO model_config (
                id
                file_name
                prompt_template
            ) VALUES (
                $1,
                $2,
                $3
            ) ON CONFLICT DO UPDATE SET
                file_name = $2,
                prompt_template = $3
            ",
        )
        .bind(model_config.id.0)
        .bind(model_config.file_name)
        .bind(model_config.prompt_template)
        .execute(&self.db_connection)
        .await?;

        Ok(())
    }

    pub async fn get_model_config_by_id(&self, model_id: &ModelId) -> Result<Option<ModelConfig>> {
        let row = sqlx::query(
            "
            SELECT
                file_name,
                prompt_template
            FROM model_config
            WHERE
                id = $1
            ",
        )
        .bind(&model_id.0)
        .map(|row: SqliteRow| ModelConfig {
            id: model_id.clone(),
            file_name: row.get("file_name"),
            prompt_template: row.get("prompt_template"),
        })
        .fetch_optional(&self.db_connection)
        .await?;

        Ok(row)
    }
}
