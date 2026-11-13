//! The sandbox policy (W22; US-2.9).

use crate::context::RequestContext;
use crate::deps::App;
use crate::error::{AppError, FieldIssue};
use tandem_core::audit::{AuditEventType, AuditOutcome};
use tandem_core::config::TestStatus;
use tandem_core::permissions::Action;
use tandem_core::sandbox_policy::{NetworkRule, PolicyScope, RunnerTier, SandboxPolicy};
use tandem_ports::StoreError;

fn network_text(network: &NetworkRule) -> String {
    match network {
        NetworkRule::None => "none".to_owned(),
        NetworkRule::AllowHosts(hosts) => hosts
            .iter()
            .map(|host| host.as_str())
            .collect::<Vec<_>>()
            .join(","),
    }
}

fn tools_text(policy: &SandboxPolicy) -> String {
    policy
        .tool_allow_list
        .iter()
        .map(|tool| tool.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

/// (name, before, after) for every field that changed.
fn changes(before: &SandboxPolicy, after: &SandboxPolicy) -> Vec<(&'static str, String, String)> {
    let limits = |policy: &SandboxPolicy| {
        let limits = &policy.limits;
        [
            ("wall_clock_seconds", limits.wall_clock_seconds),
            ("max_output_bytes", limits.max_output_bytes),
            ("max_file_bytes", limits.max_file_bytes),
            ("max_files_written", limits.max_files_written),
            ("memory_mebibytes", limits.memory_mebibytes),
            ("cpu_millicores", limits.cpu_millicores),
        ]
    };
    let mut all = vec![
        (
            "runner_tier",
            before.runner_tier.as_str().to_owned(),
            after.runner_tier.as_str().to_owned(),
        ),
        ("tools", tools_text(before), tools_text(after)),
        (
            "network",
            network_text(&before.network),
            network_text(&after.network),
        ),
    ];
    for ((name, old), (_, new)) in limits(before).into_iter().zip(limits(after)) {
        all.push((name, old.to_string(), new.to_string()));
    }
    all.into_iter().filter(|(_, old, new)| old != new).collect()
}

impl App {
    /// The instance policy: the saved one, or the factory default for an instance that never
    /// saved one (BR-18.1).
    ///
    /// # Errors
    /// [`AppError::NotFound`] for non-admins.
    pub async fn get_sandbox_policy(
        &self,
        ctx: &RequestContext,
    ) -> Result<SandboxPolicy, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let mut tx = self.store.begin().await?;
        Ok(tx
            .policies()
            .get(PolicyScope::Instance)
            .await?
            .unwrap_or_else(SandboxPolicy::factory_default))
    }

    /// Save the instance policy. The candidate's version is ignored: the saved version is one more
    /// than the one it replaces. Choosing the container tier needs a runtime that answers a fresh
    /// probe right now (BR-18.2). Runs in progress keep the policy they started with (BR-18.1).
    ///
    /// # Errors
    /// [`AppError::Validation`] naming every field that breaks a rule, or a concurrent change.
    pub async fn save_sandbox_policy(
        &self,
        ctx: &RequestContext,
        candidate: SandboxPolicy,
    ) -> Result<SandboxPolicy, AppError> {
        self.authorize(ctx, Action::ManageIntegrations).await?;
        let actor = ctx.actor()?;
        let mut issues: Vec<FieldIssue> = Vec::new();
        if candidate.scope != PolicyScope::Instance {
            issues.push(FieldIssue {
                field: "scope",
                message: "Only the instance policy can be edited".to_owned(),
            });
        }
        if let Err(found) = candidate.validate() {
            issues.extend(found.iter().map(|issue| FieldIssue {
                field: issue.field(),
                message: issue.to_string(),
            }));
        }
        if candidate.runner_tier == RunnerTier::Container && issues.is_empty() {
            let probe = self.integrations.runtime_probe.probe().await;
            if probe.status != TestStatus::Ok {
                issues.push(FieldIssue {
                    field: "runner_tier",
                    message: "The container runtime cannot be reached, so the container tier is not available".to_owned(),
                });
            }
        }
        if !issues.is_empty() {
            return Err(AppError::Validation(issues));
        }

        let now = self.clock.now();
        let mut tx = self.store.begin().await?;
        let current = tx
            .policies()
            .get(PolicyScope::Instance)
            .await?
            .unwrap_or_else(SandboxPolicy::factory_default);
        let saved = SandboxPolicy {
            version: current.version.saturating_add(1),
            ..candidate
        };
        tx.policies()
            .save(&saved, current.version, actor.user_id, now)
            .await
            .map_err(|error| match error {
                StoreError::Conflict(_) => AppError::validation(
                    "version",
                    "Someone else changed the policy. Reload and try again",
                ),
                other => other.into(),
            })?;
        let mut event = self
            .audit_event(
                ctx,
                AuditEventType::ConfigSandboxPolicyUpdated,
                AuditOutcome::Success,
            )
            .object("sandbox_policy", "instance")
            .detail("version_before", current.version.to_string())
            .detail("version_after", saved.version.to_string());
        for (name, old, new) in changes(&current, &saved) {
            event = event
                .detail(&format!("{name}_before"), old)
                .detail(&format!("{name}_after"), new);
        }
        self.append_audit(tx.as_mut(), event).await?;
        tx.commit().await?;
        self.config_cache.clear();
        Ok(saved)
    }
}
