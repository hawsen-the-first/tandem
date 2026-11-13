//! Default models, search settings, connectors and sandbox policies (U2, BR-15 to BR-18).

use crate::error::{corrupt, map_sqlx};
use crate::store::PgTx;
use async_trait::async_trait;
use std::collections::BTreeSet;
use tandem_core::config::{
    Connector, ConnectorAuth, ConnectorKind, ModelDefaults, SearchConfig, SearchKind,
};
use tandem_core::ids::{ProviderModelId, UserId};
use tandem_core::outbound_url::OutboundUrl;
use tandem_core::sandbox_policy::{
    HostName, Limits, NetworkRule, PolicyScope, RunnerTier, SandboxPolicy, ToolId,
};
use tandem_ports::{ConnectorRepo, DefaultsRepo, PolicyRepo, SearchRepo, SecretChange, StoreError};
use time::OffsetDateTime;
use uuid::Uuid;

fn key_change(change: &SecretChange) -> (&'static str, Option<&[u8]>) {
    match change {
        SecretChange::Keep => ("keep", None),
        SecretChange::Set(bytes) => ("set", Some(bytes.as_slice())),
        SecretChange::Clear => ("clear", None),
    }
}

#[async_trait]
impl DefaultsRepo for PgTx {
    async fn get(&mut self) -> Result<ModelDefaults, StoreError> {
        let row = sqlx::query!(
            "SELECT orchestrator_model_id, agent_model_id FROM model_defaults WHERE singleton"
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(
            row.map_or_else(ModelDefaults::default, |row| ModelDefaults {
                orchestrator: row.orchestrator_model_id.map(ProviderModelId::from_uuid),
                agents: row.agent_model_id.map(ProviderModelId::from_uuid),
            }),
        )
    }

    async fn save(
        &mut self,
        defaults: &ModelDefaults,
        by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO model_defaults (singleton, orchestrator_model_id, agent_model_id, updated_at, updated_by)
             VALUES (true, $1, $2, $3, $4)
             ON CONFLICT (singleton) DO UPDATE
             SET orchestrator_model_id = EXCLUDED.orchestrator_model_id,
                 agent_model_id = EXCLUDED.agent_model_id,
                 updated_at = EXCLUDED.updated_at, updated_by = EXCLUDED.updated_by",
            defaults.orchestrator.map(|id| id.as_uuid()),
            defaults.agents.map(|id| id.as_uuid()),
            now,
            by.as_uuid(),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

struct SearchRow {
    kind: String,
    base_url: Option<String>,
    key_saved_at: Option<OffsetDateTime>,
    updated_at: OffsetDateTime,
    updated_by: Uuid,
}

impl SearchRow {
    fn into_config(self) -> Result<SearchConfig, StoreError> {
        Ok(SearchConfig {
            kind: SearchKind::parse(&self.kind).ok_or_else(|| corrupt("search kind"))?,
            base_url: self
                .base_url
                .as_deref()
                .map(OutboundUrl::parse)
                .transpose()
                .map_err(|_| corrupt("search URL"))?,
            key_saved_at: self.key_saved_at,
            updated_at: self.updated_at,
            updated_by: UserId::from_uuid(self.updated_by),
        })
    }
}

#[async_trait]
impl SearchRepo for PgTx {
    async fn list(&mut self) -> Result<Vec<SearchConfig>, StoreError> {
        let rows = sqlx::query_as!(
            SearchRow,
            "SELECT kind, base_url, key_saved_at, updated_at, updated_by FROM search_configs ORDER BY kind"
        )
        .fetch_all(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        rows.into_iter().map(SearchRow::into_config).collect()
    }

    async fn get(&mut self, kind: SearchKind) -> Result<Option<SearchConfig>, StoreError> {
        let row = sqlx::query_as!(
            SearchRow,
            "SELECT kind, base_url, key_saved_at, updated_at, updated_by FROM search_configs WHERE kind = $1",
            kind.as_str(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(SearchRow::into_config).transpose()
    }

    async fn save(&mut self, config: &SearchConfig, key: SecretChange) -> Result<(), StoreError> {
        let (action, sealed) = key_change(&key);
        sqlx::query!(
            "INSERT INTO search_configs (kind, base_url, api_key_sealed, key_saved_at, updated_at, updated_by)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (kind) DO UPDATE
             SET base_url = EXCLUDED.base_url, key_saved_at = EXCLUDED.key_saved_at,
                 updated_at = EXCLUDED.updated_at, updated_by = EXCLUDED.updated_by,
                 api_key_sealed = CASE $7::text
                     WHEN 'set' THEN EXCLUDED.api_key_sealed
                     WHEN 'clear' THEN NULL
                     ELSE search_configs.api_key_sealed END",
            config.kind.as_str(),
            config.base_url.as_ref().map(OutboundUrl::as_str),
            sealed,
            config.key_saved_at,
            config.updated_at,
            config.updated_by.as_uuid(),
            action,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn sealed_key(&mut self, kind: SearchKind) -> Result<Option<Vec<u8>>, StoreError> {
        let row = sqlx::query!(
            "SELECT api_key_sealed FROM search_configs WHERE kind = $1",
            kind.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.and_then(|row| row.api_key_sealed))
    }

    async fn active(&mut self) -> Result<Option<SearchKind>, StoreError> {
        let row = sqlx::query!("SELECT kind FROM search_active WHERE singleton")
            .fetch_optional(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        row.and_then(|row| row.kind)
            .map(|text| SearchKind::parse(&text).ok_or_else(|| corrupt("search kind")))
            .transpose()
    }

    async fn set_active(
        &mut self,
        kind: Option<SearchKind>,
        by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        if let Some(kind) = kind {
            let exists = sqlx::query_scalar!(
                "SELECT EXISTS (SELECT 1 FROM search_configs WHERE kind = $1) AS \"exists!\"",
                kind.as_str()
            )
            .fetch_one(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
            if !exists {
                return Err(StoreError::NotFound);
            }
        }
        sqlx::query!(
            "INSERT INTO search_active (singleton, kind, updated_at, updated_by) VALUES (true, $1, $2, $3)
             ON CONFLICT (singleton) DO UPDATE
             SET kind = EXCLUDED.kind, updated_at = EXCLUDED.updated_at, updated_by = EXCLUDED.updated_by",
            kind.map(SearchKind::as_str),
            now,
            by.as_uuid(),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }
}

#[async_trait]
impl ConnectorRepo for PgTx {
    async fn get(&mut self, kind: ConnectorKind) -> Result<Option<Connector>, StoreError> {
        let row = sqlx::query!(
            "SELECT kind, auth_kind, secret_saved_at, account_login, token_expires_at, updated_at, updated_by
             FROM connectors WHERE kind = $1",
            kind.as_str(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| {
            Ok(Connector {
                kind: ConnectorKind::parse(&row.kind).ok_or_else(|| corrupt("connector kind"))?,
                auth: ConnectorAuth::parse(&row.auth_kind)
                    .ok_or_else(|| corrupt("connector auth"))?,
                secret_saved_at: row.secret_saved_at,
                account_login: row.account_login,
                token_expires_at: row.token_expires_at,
                updated_at: row.updated_at,
                updated_by: UserId::from_uuid(row.updated_by),
            })
        })
        .transpose()
    }

    async fn save(
        &mut self,
        connector: &Connector,
        sealed_secret: &[u8],
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "INSERT INTO connectors
                (kind, auth_kind, secret_sealed, secret_saved_at, account_login, token_expires_at, updated_at, updated_by)
             VALUES ($1, $2, $3, $4, NULL, NULL, $5, $6)
             ON CONFLICT (kind) DO UPDATE
             SET auth_kind = EXCLUDED.auth_kind, secret_sealed = EXCLUDED.secret_sealed,
                 secret_saved_at = EXCLUDED.secret_saved_at, account_login = NULL,
                 token_expires_at = NULL, updated_at = EXCLUDED.updated_at,
                 updated_by = EXCLUDED.updated_by",
            connector.kind.as_str(),
            connector.auth.as_str(),
            sealed_secret,
            connector.secret_saved_at,
            connector.updated_at,
            connector.updated_by.as_uuid(),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn sealed_secret(&mut self, kind: ConnectorKind) -> Result<Option<Vec<u8>>, StoreError> {
        let row = sqlx::query!(
            "SELECT secret_sealed FROM connectors WHERE kind = $1",
            kind.as_str()
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(row.map(|row| row.secret_sealed))
    }

    async fn set_identity(
        &mut self,
        kind: ConnectorKind,
        account_login: Option<&str>,
        token_expires_at: Option<OffsetDateTime>,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            "UPDATE connectors SET account_login = $2, token_expires_at = $3 WHERE kind = $1",
            kind.as_str(),
            account_login,
            token_expires_at,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        Ok(())
    }

    async fn delete(&mut self, kind: ConnectorKind) -> Result<(), StoreError> {
        let result = sqlx::query!("DELETE FROM connectors WHERE kind = $1", kind.as_str())
            .execute(&mut *self.tx)
            .await
            .map_err(map_sqlx)?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

fn to_stored(value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| corrupt("limit"))
}

fn from_stored(value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| corrupt("limit"))
}

#[async_trait]
impl PolicyRepo for PgTx {
    async fn get(&mut self, scope: PolicyScope) -> Result<Option<SandboxPolicy>, StoreError> {
        let row = sqlx::query!(
            "SELECT scope, runner_tier, tool_allow_list, network_mode, network_hosts,
                    wall_clock_seconds, max_output_bytes, max_file_bytes, max_files_written,
                    memory_mebibytes, cpu_millicores, version
             FROM sandbox_policies WHERE scope = $1",
            scope.as_str(),
        )
        .fetch_optional(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        row.map(|row| {
            let tools = row
                .tool_allow_list
                .iter()
                .map(|text| ToolId::parse(text).ok_or_else(|| corrupt("tool")))
                .collect::<Result<BTreeSet<_>, _>>()?;
            let network = match row.network_mode.as_str() {
                "none" => NetworkRule::None,
                "allow_hosts" => NetworkRule::AllowHosts(
                    row.network_hosts
                        .iter()
                        .map(|text| HostName::parse(text).map_err(|_| corrupt("host")))
                        .collect::<Result<BTreeSet<_>, _>>()?,
                ),
                _ => return Err(corrupt("network rule")),
            };
            Ok(SandboxPolicy {
                scope: PolicyScope::parse(&row.scope).ok_or_else(|| corrupt("policy scope"))?,
                runner_tier: RunnerTier::parse(&row.runner_tier)
                    .ok_or_else(|| corrupt("runner tier"))?,
                tool_allow_list: tools,
                network,
                limits: Limits {
                    wall_clock_seconds: from_stored(row.wall_clock_seconds)?,
                    max_output_bytes: from_stored(row.max_output_bytes)?,
                    max_file_bytes: from_stored(row.max_file_bytes)?,
                    max_files_written: from_stored(row.max_files_written)?,
                    memory_mebibytes: from_stored(row.memory_mebibytes)?,
                    cpu_millicores: from_stored(row.cpu_millicores)?,
                },
                version: u32::try_from(row.version).map_err(|_| corrupt("policy version"))?,
            })
        })
        .transpose()
    }

    async fn save(
        &mut self,
        policy: &SandboxPolicy,
        previous_version: u32,
        by: UserId,
        now: OffsetDateTime,
    ) -> Result<(), StoreError> {
        let tools: Vec<String> = policy
            .tool_allow_list
            .iter()
            .map(|tool| tool.as_str().to_owned())
            .collect();
        let (mode, hosts): (&str, Vec<String>) = match &policy.network {
            NetworkRule::None => ("none", Vec::new()),
            NetworkRule::AllowHosts(hosts) => (
                "allow_hosts",
                hosts.iter().map(|host| host.as_str().to_owned()).collect(),
            ),
        };
        let version = i32::try_from(policy.version).map_err(|_| corrupt("policy version"))?;
        let previous = i32::try_from(previous_version).map_err(|_| corrupt("policy version"))?;
        let limits = &policy.limits;
        let updated = sqlx::query!(
            "UPDATE sandbox_policies
             SET runner_tier = $2, tool_allow_list = $3, network_mode = $4, network_hosts = $5,
                 wall_clock_seconds = $6, max_output_bytes = $7, max_file_bytes = $8,
                 max_files_written = $9, memory_mebibytes = $10, cpu_millicores = $11,
                 version = $12, updated_at = $13, updated_by = $14
             WHERE scope = $1 AND version = $15",
            policy.scope.as_str(),
            policy.runner_tier.as_str(),
            &tools,
            mode,
            &hosts,
            to_stored(limits.wall_clock_seconds)?,
            to_stored(limits.max_output_bytes)?,
            to_stored(limits.max_file_bytes)?,
            to_stored(limits.max_files_written)?,
            to_stored(limits.memory_mebibytes)?,
            to_stored(limits.cpu_millicores)?,
            version,
            now,
            by.as_uuid(),
            previous,
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if updated.rows_affected() == 1 {
            return Ok(());
        }
        // No row at the version the caller read: only the unsaved factory default (version 1) may
        // be saved over nothing, and only if nobody has saved a row since.
        if previous_version != 1 {
            return Err(StoreError::Conflict("sandbox_policies.version"));
        }
        let inserted = sqlx::query!(
            "INSERT INTO sandbox_policies
                (scope, runner_tier, tool_allow_list, network_mode, network_hosts,
                 wall_clock_seconds, max_output_bytes, max_file_bytes, max_files_written,
                 memory_mebibytes, cpu_millicores, version, updated_at, updated_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
             ON CONFLICT (scope) DO NOTHING",
            policy.scope.as_str(),
            policy.runner_tier.as_str(),
            &tools,
            mode,
            &hosts,
            to_stored(limits.wall_clock_seconds)?,
            to_stored(limits.max_output_bytes)?,
            to_stored(limits.max_file_bytes)?,
            to_stored(limits.max_files_written)?,
            to_stored(limits.memory_mebibytes)?,
            to_stored(limits.cpu_millicores)?,
            version,
            now,
            by.as_uuid(),
        )
        .execute(&mut *self.tx)
        .await
        .map_err(map_sqlx)?;
        if inserted.rows_affected() == 1 {
            Ok(())
        } else {
            Err(StoreError::Conflict("sandbox_policies.version"))
        }
    }
}
