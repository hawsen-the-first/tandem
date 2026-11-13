//! Providers and their models (U2, BR-14).

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use tandem_core::config::{
    ModelName, Provider, ProviderKind, ProviderModel, ProviderName, ToolCalling,
};
use tandem_core::ids::{ProviderId, ProviderModelId, UserId};
use tandem_core::outbound_url::OutboundUrl;
use tandem_ports::{ProviderRepo, SecretChange, StoreError, capabilities_from};
use time::OffsetDateTime;
use uuid::Uuid;

struct ProviderRow {
    id: Uuid,
    kind: String,
    name: String,
    base_url: String,
    key_saved_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    created_by: Uuid,
    updated_at: OffsetDateTime,
    updated_by: Uuid,
}

impl ProviderRow {
    fn into_provider(self) -> Result<Provider, StoreError> {
        Ok(Provider {
            id: ProviderId::from_uuid(self.id),
            kind: ProviderKind::parse(&self.kind).ok_or_else(|| corrupt("provider kind"))?,
            name: ProviderName::parse(&self.name).map_err(|_| corrupt("provider name"))?,
            base_url: OutboundUrl::parse(&self.base_url).map_err(|_| corrupt("provider URL"))?,
            key_saved_at: self.key_saved_at,
            created_at: self.created_at,
            created_by: UserId::from_uuid(self.created_by),
            updated_at: self.updated_at,
            updated_by: UserId::from_uuid(self.updated_by),
        })
    }
}

struct ModelRow {
    id: Uuid,
    provider_id: Uuid,
    model_name: String,
    label: Option<String>,
    detected_tool_calling: String,
    declared_tool_calling: Option<String>,
    detected_context_tokens: Option<i32>,
    declared_context_tokens: Option<i32>,
}

fn tool_calling(text: &str) -> Result<ToolCalling, StoreError> {
    ToolCalling::parse(text).ok_or_else(|| corrupt("tool calling"))
}

fn tokens(stored: Option<i32>) -> Result<Option<u32>, StoreError> {
    stored
        .map(|value| u32::try_from(value).map_err(|_| corrupt("context length")))
        .transpose()
}

impl ModelRow {
    fn into_model(self) -> Result<ProviderModel, StoreError> {
        Ok(ProviderModel {
            id: ProviderModelId::from_uuid(self.id),
            provider_id: ProviderId::from_uuid(self.provider_id),
            name: ModelName::parse(&self.model_name).map_err(|_| corrupt("model name"))?,
            label: self.label,
            capabilities: capabilities_from(
                tool_calling(&self.detected_tool_calling)?,
                self.declared_tool_calling
                    .as_deref()
                    .map(tool_calling)
                    .transpose()?,
                tokens(self.detected_context_tokens)?,
                tokens(self.declared_context_tokens)?,
            ),
        })
    }
}

fn stored_tokens(value: Option<u32>) -> Result<Option<i32>, StoreError> {
    value
        .map(|tokens| i32::try_from(tokens).map_err(|_| corrupt("context length")))
        .transpose()
}

#[async_trait]
impl ProviderRepo for PgTx {
    async fn insert(
        &mut self,
        provider: &Provider,
        sealed_key: Option<&[u8]>,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO providers
                (id, kind, name, base_url, api_key_sealed, key_saved_at,
                 created_at, created_by, updated_at, updated_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            provider.id.as_uuid(),
            provider.kind.as_str(),
            provider.name.as_str(),
            provider.base_url.as_str(),
            sealed_key,
            provider.key_saved_at,
            provider.created_at,
            provider.created_by.as_uuid(),
            provider.updated_at,
            provider.updated_by.as_uuid(),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn update(&mut self, provider: &Provider, key: SecretChange) -> Result<(), StoreError> {
        let (action, sealed): (&str, Option<&[u8]>) = match &key {
            SecretChange::Keep => ("keep", None),
            SecretChange::Set(bytes) => ("set", Some(bytes.as_slice())),
            SecretChange::Clear => ("clear", None),
        };
        let result = sqlx::query!(
            "UPDATE providers
             SET name = $2, base_url = $3, key_saved_at = $4, updated_at = $5, updated_by = $6,
                 api_key_sealed = CASE $7::text
                     WHEN 'set' THEN $8::bytea
                     WHEN 'clear' THEN NULL
                     ELSE api_key_sealed END
             WHERE id = $1",
            provider.id.as_uuid(),
            provider.name.as_str(),
            provider.base_url.as_str(),
            provider.key_saved_at,
            provider.updated_at,
            provider.updated_by.as_uuid(),
            action,
            sealed,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn get(&mut self, id: ProviderId) -> Result<Option<Provider>, StoreError> {
        let row = sqlx::query_as!(
            ProviderRow,
            "SELECT id, kind, name, base_url, key_saved_at, created_at, created_by, updated_at, updated_by
             FROM providers WHERE id = $1",
            id.as_uuid(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(ProviderRow::into_provider).transpose()
    }

    async fn list(&mut self) -> Result<Vec<Provider>, StoreError> {
        let rows = sqlx::query_as!(
            ProviderRow,
            "SELECT id, kind, name, base_url, key_saved_at, created_at, created_by, updated_at, updated_by
             FROM providers ORDER BY lower(name), id",
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(ProviderRow::into_provider).collect()
    }

    async fn sealed_key(&mut self, id: ProviderId) -> Result<Option<Vec<u8>>, StoreError> {
        let row = sqlx::query!(
            "SELECT api_key_sealed FROM providers WHERE id = $1",
            id.as_uuid()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.and_then(|row| row.api_key_sealed))
    }

    async fn delete(&mut self, id: ProviderId) -> Result<(), StoreError> {
        let result = sqlx::query!("DELETE FROM providers WHERE id = $1", id.as_uuid())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    async fn models(&mut self, provider_id: ProviderId) -> Result<Vec<ProviderModel>, StoreError> {
        let rows = sqlx::query_as!(
            ModelRow,
            "SELECT id, provider_id, model_name, label, detected_tool_calling, declared_tool_calling,
                    detected_context_tokens, declared_context_tokens
             FROM provider_models WHERE provider_id = $1 ORDER BY position, id",
            provider_id.as_uuid(),
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(ModelRow::into_model).collect()
    }

    async fn all_models(&mut self) -> Result<Vec<ProviderModel>, StoreError> {
        let rows = sqlx::query_as!(
            ModelRow,
            "SELECT model.id, model.provider_id, model.model_name, model.label,
                    model.detected_tool_calling, model.declared_tool_calling,
                    model.detected_context_tokens, model.declared_context_tokens
             FROM provider_models model
             JOIN providers provider ON provider.id = model.provider_id
             ORDER BY lower(provider.name), provider.id, model.position, model.id",
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(ModelRow::into_model).collect()
    }

    async fn get_model(
        &mut self,
        id: ProviderModelId,
    ) -> Result<Option<ProviderModel>, StoreError> {
        let row = sqlx::query_as!(
            ModelRow,
            "SELECT id, provider_id, model_name, label, detected_tool_calling, declared_tool_calling,
                    detected_context_tokens, declared_context_tokens
             FROM provider_models WHERE id = $1",
            id.as_uuid(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(ModelRow::into_model).transpose()
    }

    async fn replace_models(
        &mut self,
        provider_id: ProviderId,
        models: &[ProviderModel],
    ) -> Result<Vec<ProviderModelId>, StoreError> {
        let existing = sqlx::query_scalar!(
            "SELECT id FROM provider_models WHERE provider_id = $1",
            provider_id.as_uuid()
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        let keep: Vec<Uuid> = models.iter().map(|model| model.id.as_uuid()).collect();
        let removed: Vec<Uuid> = existing
            .into_iter()
            .filter(|id| !keep.contains(id))
            .collect();
        sqlx::query!(
            "DELETE FROM provider_models WHERE provider_id = $1 AND id = ANY($2)",
            provider_id.as_uuid(),
            &removed,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;

        for (position, model) in models.iter().enumerate() {
            let position = i32::try_from(position).map_err(|_| corrupt("model position"))?;
            sqlx::query!(
                "INSERT INTO provider_models
                    (id, provider_id, model_name, label, position,
                     detected_tool_calling, declared_tool_calling,
                     detected_context_tokens, declared_context_tokens)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                 ON CONFLICT (id) DO UPDATE
                 SET model_name = EXCLUDED.model_name, label = EXCLUDED.label,
                     position = EXCLUDED.position,
                     declared_tool_calling = EXCLUDED.declared_tool_calling,
                     declared_context_tokens = EXCLUDED.declared_context_tokens
                 WHERE provider_models.provider_id = EXCLUDED.provider_id",
                model.id.as_uuid(),
                provider_id.as_uuid(),
                model.name.as_str(),
                model.label.as_deref(),
                position,
                model.capabilities.detected_tool_calling.as_str(),
                model
                    .capabilities
                    .declared_tool_calling
                    .map(ToolCalling::as_str),
                stored_tokens(model.capabilities.detected_context_tokens)?,
                stored_tokens(model.capabilities.declared_context_tokens)?,
            )
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        }
        Ok(removed
            .into_iter()
            .map(ProviderModelId::from_uuid)
            .collect())
    }

    async fn set_detected(
        &mut self,
        id: ProviderModelId,
        tool_calling: ToolCalling,
        context_tokens: Option<u32>,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE provider_models
             SET detected_tool_calling = $2,
                 detected_context_tokens = COALESCE($3, detected_context_tokens)
             WHERE id = $1",
            id.as_uuid(),
            tool_calling.as_str(),
            stored_tokens(context_tokens)?,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}
