import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useId, useState } from 'react';
import { call, client } from '../../../../api/client';
import { errorMessage } from '../../../../api/messages';
import { CapabilityWarning } from '../../../../components/integrations/CapabilityWarning';
import { FormField } from '../../../../components/FormField';
import { useToast } from '../../../../components/Toast';
import { ti } from '../../../../i18n/messages.integrations';
import { cx } from '../../../../styles/cx';
import shared from '../../../../styles/shared.module.css';
import adminStyles from '../../admin.module.css';
import type { Defaults, Model, Provider } from '../api';
import { DEFAULTS_KEY } from '../api';
import styles from '../integrations.module.css';

type Slot = 'orchestrator' | 'agents';

interface Choice {
  model: Model;
  providerName: string;
}

function choices(providers: Provider[]): Map<string, Choice> {
  const all = new Map<string, Choice>();
  for (const provider of providers) {
    for (const model of provider.models) {
      all.set(model.id, { model, providerName: provider.name });
    }
  }
  return all;
}

interface DefaultModelsCardProps {
  providers: Provider[];
  defaults: Defaults;
}

/**
 * The two default models (BR-15). Each select names the provider; an unsuitable pick shows why
 * and needs "Use anyway" before it can be saved.
 */
export function DefaultModelsCard({ providers, defaults }: DefaultModelsCardProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const headingId = useId();
  const all = choices(providers);
  const savedIds: Record<Slot, string> = {
    orchestrator: defaults.orchestrator?.model.id ?? '',
    agents: defaults.agents?.model.id ?? '',
  };
  const [picked, setPicked] = useState<Record<Slot, string>>(savedIds);
  const [useAnyway, setUseAnyway] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  // A pick whose model has gone (removed elsewhere) falls back to what the server says.
  const current: Record<Slot, string> = {
    orchestrator: all.has(picked.orchestrator) ? picked.orchestrator : savedIds.orchestrator,
    agents: all.has(picked.agents) ? picked.agents : savedIds.agents,
  };
  const dirty =
    current.orchestrator !== savedIds.orchestrator || current.agents !== savedIds.agents;
  const changedWithWarnings = (['orchestrator', 'agents'] as const).some(
    (slot) =>
      current[slot] !== savedIds[slot] && (all.get(current[slot])?.model.warnings.length ?? 0) > 0,
  );

  const save = useMutation({
    mutationFn: () =>
      call(
        client.PUT('/api/admin/model-defaults', {
          body: {
            orchestrator_model_id: current.orchestrator === '' ? null : current.orchestrator,
            agents_model_id: current.agents === '' ? null : current.agents,
          },
        }),
      ),
    onSuccess: (saved) => {
      queryClient.setQueryData(DEFAULTS_KEY, saved);
      setUseAnyway(false);
      toast.show('success', ti('defaultsSaved'));
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  const slotField = (slot: Slot, label: string) => {
    const choice = all.get(current[slot]);
    return (
      <FormField
        label={label}
        hint={
          choice === undefined ? undefined : ti('defaultsFrom', { provider: choice.providerName })
        }
        testId={`defaults-${slot}`}
      >
        {(control) => (
          <>
            <select
              {...control}
              className={styles.select}
              value={current[slot]}
              data-testid={`defaults-${slot}-input`}
              onChange={(event) => {
                setPicked({ ...current, [slot]: event.target.value });
                setProblem(null);
              }}
            >
              <option value="">{ti('defaultsNone')}</option>
              {providers.map((provider) => (
                <optgroup key={provider.id} label={provider.name}>
                  {provider.models.map((model) => (
                    <option key={model.id} value={model.id}>
                      {ti('defaultsOption', {
                        provider: provider.name,
                        model: model.label ?? model.name,
                      })}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
            {choice === undefined ? null : (
              <CapabilityWarning
                warnings={choice.model.warnings}
                testId={`defaults-${slot}-warnings`}
              />
            )}
          </>
        )}
      </FormField>
    );
  };

  return (
    <section
      className={cx(shared.card, styles.section)}
      aria-labelledby={headingId}
      data-testid="defaults-card"
    >
      <h2 id={headingId}>{ti('defaultsTitle')}</h2>
      <p className={shared.muted}>{ti('defaultsIntro')}</p>
      {all.size === 0 ? (
        <p data-testid="defaults-no-models">{ti('defaultsNoModels')}</p>
      ) : (
        <form
          className={cx(styles.form, styles.formNarrow)}
          noValidate
          onSubmit={(event) => {
            event.preventDefault();
            setFailure(null);
            if (changedWithWarnings && !useAnyway) {
              setProblem(ti('defaultsConfirmNeeded'));
              return;
            }
            save.mutate();
          }}
        >
          {slotField('orchestrator', ti('defaultsOrchestrator'))}
          {slotField('agents', ti('defaultsAgents'))}
          {changedWithWarnings ? (
            <div>
              <label className={adminStyles.checkboxRow}>
                <input
                  type="checkbox"
                  checked={useAnyway}
                  aria-invalid={problem === null ? undefined : true}
                  data-testid="defaults-use-anyway"
                  onChange={(event) => {
                    setUseAnyway(event.target.checked);
                    setProblem(null);
                  }}
                />
                <span>{ti('defaultsUseAnyway')}</span>
              </label>
              {problem === null ? null : (
                <p
                  className={cx(shared.banner, shared.bannerError)}
                  role="alert"
                  data-testid="defaults-problem"
                >
                  {problem}
                </p>
              )}
            </div>
          ) : null}
          {failure === null ? null : (
            <p
              role="alert"
              className={cx(shared.banner, shared.bannerError)}
              data-testid="defaults-error"
            >
              {failure}
            </p>
          )}
          <div>
            <button
              type="submit"
              className={shared.button}
              disabled={!dirty || save.isPending}
              data-testid="defaults-save"
            >
              {ti('defaultsSave')}
            </button>
          </div>
        </form>
      )}
    </section>
  );
}
