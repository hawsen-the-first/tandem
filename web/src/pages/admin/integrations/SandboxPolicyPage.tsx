import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useId, useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage, fieldErrors } from '../../../api/messages';
import { useTestRun } from '../../../api/useTestRun';
import { FormField } from '../../../components/FormField';
import { HostListEditor } from '../../../components/integrations/HostListEditor';
import { TestResultView } from '../../../components/integrations/TestResultView';
import { checkHosts } from '../../../components/integrations/outboundRules';
import { useToast } from '../../../components/Toast';
import { t } from '../../../i18n/messages';
import { ti } from '../../../i18n/messages.integrations';
import type { IntegrationsMessageKey } from '../../../i18n/messages.integrations';
import { cx } from '../../../styles/cx';
import shared from '../../../styles/shared.module.css';
import componentStyles from '../../../components/integrations/integrations.module.css';
import adminStyles from '../admin.module.css';
import type { Limits, PolicyView } from './api';
import { POLICY_KEY } from './api';
import styles from './integrations.module.css';

type LimitField = keyof Limits;

export const LIMIT_FIELDS: {
  field: LimitField;
  label: IntegrationsMessageKey;
  unit: IntegrationsMessageKey;
}[] = [
  { field: 'wall_clock_seconds', label: 'limitWallClock', unit: 'unitSeconds' },
  { field: 'max_output_bytes', label: 'limitOutput', unit: 'unitBytes' },
  { field: 'max_file_bytes', label: 'limitFileSize', unit: 'unitBytes' },
  { field: 'max_files_written', label: 'limitFileCount', unit: 'unitFiles' },
  { field: 'memory_mebibytes', label: 'limitMemory', unit: 'unitMebibytes' },
  { field: 'cpu_millicores', label: 'limitCpu', unit: 'unitMillicores' },
];

interface Draft {
  tier: string;
  tools: string[];
  networkMode: 'none' | 'allow_hosts';
  hosts: string[];
  limits: Record<LimitField, string>;
}

interface Problems {
  tier?: string;
  tools?: string;
  network?: string;
  hosts: Record<number, string>;
  limits: Partial<Record<LimitField, string>>;
}

function draftFrom(view: PolicyView): Draft {
  const { policy } = view;
  const limits = Object.fromEntries(
    LIMIT_FIELDS.map(({ field }) => [field, String(policy.limits[field])]),
  ) as Record<LimitField, string>;
  return {
    tier: policy.runner_tier,
    tools: [...policy.tool_allow_list],
    networkMode: policy.network.mode === 'allow_hosts' ? 'allow_hosts' : 'none',
    hosts: [...(policy.network.hosts ?? [])],
    limits,
  };
}

function sameDraft(a: Draft, b: Draft): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

function rangeFor(view: PolicyView, field: LimitField) {
  return view.ranges.find((range) => range.field === field);
}

/** Client checks for the policy (BR-18.3 to BR-18.5); the server checks again. */
export function checkPolicy(view: PolicyView, draft: Draft): Problems {
  const problems: Problems = { hosts: {}, limits: {} };
  for (const { field, unit } of LIMIT_FIELDS) {
    const range = rangeFor(view, field);
    const text = draft.limits[field].trim();
    const value = /^\d+$/.test(text) ? Number(text) : NaN;
    if (
      range !== undefined &&
      (!Number.isSafeInteger(value) || value < range.min || value > range.max)
    ) {
      problems.limits[field] = ti('sandboxLimitInvalid', {
        min: range.min.toLocaleString(),
        max: range.max.toLocaleString(),
        unit: ti(unit),
      });
    }
  }
  if (draft.networkMode === 'allow_hosts') {
    problems.hosts = checkHosts(draft.hosts);
  }
  return problems;
}

function hasProblems(problems: Problems): boolean {
  return (
    problems.tier !== undefined ||
    problems.tools !== undefined ||
    problems.network !== undefined ||
    Object.keys(problems.hosts).length > 0 ||
    Object.keys(problems.limits).length > 0
  );
}

function runtimeAvailable(run: ReturnType<typeof useTestRun>['run']): boolean {
  return run?.status === 'succeeded' && run.results.some((result) => result.status === 'ok');
}

function PolicyForm({ view }: { view: PolicyView }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const tierId = useId();
  const networkId = useId();
  const toolsId = useId();
  const limitsId = useId();
  const baseline = draftFrom(view);
  const [draft, setDraft] = useState<Draft>(baseline);
  const [problems, setProblems] = useState<Problems>({ hosts: {}, limits: {} });
  const [failure, setFailure] = useState<string | null>(null);
  const dirty = !sameDraft(draft, baseline);

  const start = useCallback(() => call(client.POST('/api/admin/runtime/check')), []);
  const runtime = useTestRun({ start });
  const containerOffered = runtimeAvailable(runtime.run) || view.policy.runner_tier === 'container';

  const save = useMutation({
    mutationFn: () =>
      call(
        client.PUT('/api/admin/sandbox-policy', {
          body: {
            runner_tier: draft.tier,
            tool_allow_list: draft.tools,
            network:
              draft.networkMode === 'none'
                ? { mode: 'none', hosts: [] }
                : {
                    mode: 'allow_hosts',
                    hosts: draft.hosts.map((host) => host.trim().toLowerCase()),
                  },
            limits: Object.fromEntries(
              LIMIT_FIELDS.map(({ field }) => [field, Number(draft.limits[field].trim())]),
            ) as Limits,
          },
        }),
      ),
    onSuccess: (saved) => {
      queryClient.setQueryData(POLICY_KEY, saved);
      toast.show('success', ti('sandboxSaved'));
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      const found: Problems = { hosts: {}, limits: {} };
      if (fields['runner_tier']) found.tier = fields['runner_tier'];
      if (fields['tool_allow_list']) found.tools = fields['tool_allow_list'];
      if (fields['network']) found.network = fields['network'];
      for (const { field } of LIMIT_FIELDS) {
        if (fields[field]) found.limits[field] = fields[field];
      }
      setProblems(found);
      setFailure(errorMessage(error));
    },
  });

  return (
    <form
      className={cx(styles.form, styles.formNarrow)}
      noValidate
      data-testid="policy-form"
      onSubmit={(event) => {
        event.preventDefault();
        setFailure(null);
        const found = checkPolicy(view, draft);
        setProblems(found);
        if (hasProblems(found)) {
          return;
        }
        save.mutate();
      }}
    >
      <p className={cx(shared.banner, shared.bannerWarning)} data-testid="policy-applies">
        {ti('sandboxApplies')}
      </p>

      <fieldset
        className={componentStyles.fieldset}
        aria-describedby={problems.tier ? `${tierId}-error` : undefined}
      >
        <legend>{ti('sandboxTier')}</legend>
        <label className={styles.radioRow}>
          <input
            type="radio"
            name={tierId}
            value="in_process"
            checked={draft.tier === 'in_process'}
            data-testid="policy-tier-in_process"
            onChange={() => {
              setDraft({ ...draft, tier: 'in_process' });
            }}
          />
          <span>
            {ti('sandboxTierInProcess')}
            <span className={adminStyles.checkboxHint}>{ti('sandboxTierInProcessHint')}</span>
          </span>
        </label>
        <label className={styles.radioRow}>
          <input
            type="radio"
            name={tierId}
            value="container"
            checked={draft.tier === 'container'}
            disabled={!containerOffered}
            aria-describedby={`${tierId}-container-hint`}
            data-testid="policy-tier-container"
            onChange={() => {
              setDraft({ ...draft, tier: 'container' });
            }}
          />
          <span>
            {ti('sandboxTierContainer')}
            <span id={`${tierId}-container-hint`} className={adminStyles.checkboxHint}>
              {containerOffered ? ti('sandboxTierContainerHint') : ti('sandboxTierContainerLocked')}
            </span>
          </span>
        </label>
        <div>
          <button
            type="button"
            className={componentStyles.smallButton}
            disabled={runtime.running}
            data-testid="policy-check-runtime"
            onClick={runtime.begin}
          >
            {ti('sandboxCheckRuntime')}
          </button>
        </div>
        {runtime.run !== undefined || runtime.running || runtime.error ? (
          <>
            <TestResultView
              run={runtime.run}
              running={runtime.running}
              timedOut={runtime.timedOut}
              error={runtime.error}
              heading={ti('sandboxCheckRuntime')}
              testId="policy-runtime"
            />
            {!runtime.running && runtime.run !== undefined && !runtimeAvailable(runtime.run) ? (
              <p className={componentStyles.hint} data-testid="policy-runtime-unavailable">
                {ti('sandboxRuntimeFailed')}
              </p>
            ) : null}
          </>
        ) : null}
        {problems.tier ? (
          <p id={`${tierId}-error`} className={componentStyles.rowError}>
            <span aria-hidden="true">⚠ </span>
            {problems.tier}
          </p>
        ) : null}
      </fieldset>

      <fieldset
        className={componentStyles.fieldset}
        aria-describedby={problems.tools ? `${toolsId}-error` : undefined}
        data-testid="policy-tools"
      >
        <legend>{ti('sandboxTools')}</legend>
        <div className={styles.toolList}>
          {view.tools.map((tool) => {
            const hintId = `${toolsId}-${tool.id}`;
            return (
              <label key={tool.id} className={adminStyles.checkboxRow}>
                <input
                  type="checkbox"
                  checked={tool.can_be_enabled && draft.tools.includes(tool.id)}
                  disabled={!tool.can_be_enabled}
                  aria-describedby={hintId}
                  data-testid={`policy-tool-${tool.id}`}
                  onChange={(event) => {
                    setDraft({
                      ...draft,
                      tools: event.target.checked
                        ? [...draft.tools, tool.id]
                        : draft.tools.filter((id) => id !== tool.id),
                    });
                  }}
                />
                <span>
                  <code>{tool.id}</code>
                  <span id={hintId} className={adminStyles.checkboxHint}>
                    {tool.can_be_enabled ? tool.description : ti('sandboxToolDisabled')}
                  </span>
                </span>
              </label>
            );
          })}
        </div>
        {problems.tools ? (
          <p id={`${toolsId}-error`} className={componentStyles.rowError}>
            <span aria-hidden="true">⚠ </span>
            {problems.tools}
          </p>
        ) : null}
      </fieldset>

      <fieldset
        className={componentStyles.fieldset}
        aria-describedby={`${networkId}-note`}
        data-testid="policy-network"
      >
        <legend>{ti('sandboxNetwork')}</legend>
        <p id={`${networkId}-note`} className={componentStyles.hint}>
          {ti('sandboxNetworkNote')}
        </p>
        <label className={styles.radioRow}>
          <input
            type="radio"
            name={networkId}
            checked={draft.networkMode === 'none'}
            data-testid="policy-network-none"
            onChange={() => {
              setDraft({ ...draft, networkMode: 'none' });
            }}
          />
          <span>{ti('sandboxNetworkNone')}</span>
        </label>
        <label className={styles.radioRow}>
          <input
            type="radio"
            name={networkId}
            checked={draft.networkMode === 'allow_hosts'}
            data-testid="policy-network-hosts"
            onChange={() => {
              setDraft({
                ...draft,
                networkMode: 'allow_hosts',
                hosts: draft.hosts.length === 0 ? [''] : draft.hosts,
              });
            }}
          />
          <span>{ti('sandboxNetworkHosts')}</span>
        </label>
        {draft.networkMode === 'allow_hosts' ? (
          <HostListEditor
            hosts={draft.hosts}
            errors={problems.hosts}
            testId="policy-hosts"
            onChange={(hosts) => {
              setDraft({ ...draft, hosts });
            }}
          />
        ) : null}
        {problems.network ? (
          <p className={componentStyles.rowError}>
            <span aria-hidden="true">⚠ </span>
            {problems.network}
          </p>
        ) : null}
      </fieldset>

      <fieldset
        className={componentStyles.fieldset}
        aria-describedby={`${limitsId}-note`}
        data-testid="policy-limits"
      >
        <legend>{ti('sandboxLimits')}</legend>
        <p id={`${limitsId}-note`} className={componentStyles.hint}>
          {ti('sandboxLimitsNote')}
        </p>
        {LIMIT_FIELDS.map(({ field, label, unit }) => {
          const range = rangeFor(view, field);
          const factory = view.factory.limits[field];
          return (
            <div key={field} className={styles.limitRow}>
              <FormField
                label={`${ti(label)} (${ti(unit)})`}
                hint={
                  range === undefined
                    ? undefined
                    : ti('sandboxLimitRange', {
                        default: range.default.toLocaleString(),
                        min: range.min.toLocaleString(),
                        max: range.max.toLocaleString(),
                        unit: ti(unit),
                      })
                }
                error={problems.limits[field]}
                required
                testId={`policy-limit-${field}`}
              >
                {(control) => (
                  <input
                    {...control}
                    inputMode="numeric"
                    value={draft.limits[field]}
                    data-testid={`policy-limit-${field}-input`}
                    onChange={(event) => {
                      setDraft({
                        ...draft,
                        limits: { ...draft.limits, [field]: event.target.value },
                      });
                    }}
                  />
                )}
              </FormField>
              <button
                type="button"
                className={componentStyles.smallButton}
                aria-label={ti('sandboxResetFor', { label: ti(label) })}
                disabled={draft.limits[field] === String(factory)}
                data-testid={`policy-reset-${field}`}
                onClick={() => {
                  setDraft({ ...draft, limits: { ...draft.limits, [field]: String(factory) } });
                }}
              >
                {ti('sandboxReset')}
              </button>
            </div>
          );
        })}
      </fieldset>

      {failure === null ? null : (
        <p
          role="alert"
          className={cx(shared.banner, shared.bannerError)}
          data-testid="policy-error"
        >
          {failure}
        </p>
      )}
      <div className={styles.actionsRow}>
        <button
          type="submit"
          className={shared.button}
          disabled={!dirty || save.isPending}
          data-testid="policy-save"
        >
          {ti('sandboxSave')}
        </button>
        <span className={shared.muted}>
          {ti('sandboxVersion', { version: view.policy.version })}
        </span>
        {dirty ? (
          <span role="status" data-testid="policy-dirty">
            {ti('sandboxUnsaved')}
          </span>
        ) : null}
      </div>
    </form>
  );
}

export function Component() {
  const policy = useQuery({
    queryKey: POLICY_KEY,
    queryFn: () => call(client.GET('/api/admin/sandbox-policy')),
  });
  return (
    <>
      <h1>{ti('sandboxTitle')}</h1>
      <p className={shared.muted}>{ti('sandboxIntro')}</p>
      {policy.isPending ? (
        <p role="status" data-testid="policy-loading">
          {t('loading')}
        </p>
      ) : policy.isError ? (
        <div
          role="alert"
          className={cx(shared.banner, shared.bannerError)}
          data-testid="policy-load-error"
        >
          <p>{ti('sandboxLoadError')}</p>
          <p>{errorMessage(policy.error)}</p>
          <button
            type="button"
            className={shared.button}
            onClick={() => {
              void policy.refetch();
            }}
          >
            {t('retry')}
          </button>
        </div>
      ) : (
        <PolicyForm key={policy.data.policy.version} view={policy.data} />
      )}
    </>
  );
}
