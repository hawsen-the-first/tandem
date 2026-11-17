import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useId } from 'react';
import { call, client } from '../../../../api/client';
import { useTestRun } from '../../../../api/useTestRun';
import { CapabilityWarning } from '../../../../components/integrations/CapabilityWarning';
import { TestResultView } from '../../../../components/integrations/TestResultView';
import { ti } from '../../../../i18n/messages.integrations';
import { cx } from '../../../../styles/cx';
import shared from '../../../../styles/shared.module.css';
import type { Provider } from '../api';
import { DEFAULTS_KEY, PROVIDERS_KEY } from '../api';
import styles from '../integrations.module.css';

interface ProviderTestPanelProps {
  provider: Provider;
  /** Rises each time the admin asks for a test; 0 shows the last result without starting one. */
  request: number;
  /** Reports whether a test is in progress, so the table can disable its button. */
  onRunningChange: (running: boolean) => void;
}

/**
 * A provider's test: starts a background run when asked, follows it, and shows per-model results
 * as they arrive, with a warning beside any model that lacks a needed capability (US-2.3).
 */
export function ProviderTestPanel({ provider, request, onRunningChange }: ProviderTestPanelProps) {
  const queryClient = useQueryClient();
  const headingId = useId();
  const start = useCallback(
    () =>
      call(
        client.POST('/api/admin/providers/{id}/test', {
          params: { path: { id: provider.id } },
        }),
      ),
    [provider.id],
  );
  const test = useTestRun({
    start,
    activeRunId: provider.active_test_id,
    onFinished: () => {
      void queryClient.invalidateQueries({ queryKey: PROVIDERS_KEY });
      void queryClient.invalidateQueries({ queryKey: DEFAULTS_KEY });
    },
  });
  const { begin, running } = test;

  useEffect(() => {
    if (request > 0) {
      begin();
    }
  }, [request, begin]);

  useEffect(() => {
    onRunningChange(running);
  }, [running, onRunningChange]);

  useEffect(
    () => () => {
      onRunningChange(false);
    },
    [onRunningChange],
  );

  const modelName = (id: string | null | undefined) => {
    const model = provider.models.find((candidate) => candidate.id === id);
    return model?.label ?? model?.name ?? '';
  };
  const warned = provider.models.filter((model) => model.warnings.length > 0);

  return (
    <section
      className={cx(shared.card, styles.section)}
      aria-labelledby={headingId}
      data-testid="provider-test-panel"
    >
      <h2 id={headingId}>{ti('testPanelHeading', { name: provider.name })}</h2>
      <TestResultView
        run={test.run ?? (running ? undefined : provider.last_test)}
        running={running}
        timedOut={test.timedOut}
        error={test.error}
        heading={ti('testPanelHeading', { name: provider.name })}
        labelFor={(result) =>
          result.subject === 'model'
            ? ti('resultModel', { name: modelName(result.model_id) })
            : ti('resultProvider')
        }
        testId="provider-test"
      />
      {!running && warned.length > 0 ? (
        <ul className={shared.stack} data-testid="provider-test-warnings">
          {warned.map((model) => (
            <li key={model.id}>
              <strong>{model.label ?? model.name}</strong>
              <CapabilityWarning warnings={model.warnings} />
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
