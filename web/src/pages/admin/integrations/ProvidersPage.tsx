import { useQuery } from '@tanstack/react-query';
import { useCallback, useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage } from '../../../api/messages';
import { ResponsiveTable } from '../../../components/ResponsiveTable';
import type { Column } from '../../../components/ResponsiveTable';
import { StatusBadge } from '../../../components/integrations/StatusBadge';
import { runStatus } from '../../../components/integrations/TestResultView';
import { t } from '../../../i18n/messages';
import { ti } from '../../../i18n/messages.integrations';
import { cx } from '../../../styles/cx';
import shared from '../../../styles/shared.module.css';
import adminStyles from '../admin.module.css';
import { FormattedTime } from '../shared';
import type { Provider } from './api';
import { ACTIVE_TEST_REFRESH_MS, DEFAULTS_KEY, PROVIDERS_KEY } from './api';
import styles from './integrations.module.css';
import { DefaultModelsCard } from './providers/DefaultModelsCard';
import { ProviderDialog } from './providers/ProviderDialog';
import { ProviderTestPanel } from './providers/ProviderTestPanel';
import { RemoveProviderDialog } from './providers/RemoveProviderDialog';

interface DialogState {
  provider: Provider | null;
  open: boolean;
  /** A fresh form each time the dialog opens. */
  instance: number;
}

interface TestState {
  providerId: string;
  request: number;
}

function LastTest({ provider, running }: { provider: Provider; running: boolean }) {
  if (running || provider.active_test_id) {
    return <StatusBadge status="running" />;
  }
  const last = provider.last_test;
  if (last === null || last === undefined) {
    return <StatusBadge status="unknown" />;
  }
  return (
    <span className={styles.cellStack}>
      <StatusBadge status={runStatus(last)} />
      <FormattedTime iso={last.finished_at ?? last.requested_at} />
    </span>
  );
}

export function Component() {
  const providers = useQuery({
    queryKey: PROVIDERS_KEY,
    queryFn: () => call(client.GET('/api/admin/providers')),
    refetchInterval: (query) =>
      query.state.data?.some((provider) => provider.active_test_id)
        ? ACTIVE_TEST_REFRESH_MS
        : false,
  });
  const defaults = useQuery({
    queryKey: DEFAULTS_KEY,
    queryFn: () => call(client.GET('/api/admin/model-defaults')),
  });
  const [dialog, setDialog] = useState<DialogState>({ provider: null, open: false, instance: 0 });
  const [removing, setRemoving] = useState<Provider | null>(null);
  const [test, setTest] = useState<TestState | null>(null);
  const [testRunning, setTestRunning] = useState(false);
  const onRunningChange = useCallback((running: boolean) => {
    setTestRunning(running);
  }, []);

  const openDialog = (provider: Provider | null) => {
    setDialog((current) => ({ provider, open: true, instance: current.instance + 1 }));
  };
  const tested =
    test === null ? undefined : providers.data?.find((provider) => provider.id === test.providerId);

  const columns: Column<Provider>[] = [
    { key: 'name', header: ti('colProvider'), render: (row) => <strong>{row.name}</strong> },
    { key: 'kind', header: ti('colKind'), render: () => ti('kindOpenAi') },
    {
      key: 'url',
      header: ti('colBaseUrl'),
      render: (row) => <span className={styles.url}>{row.base_url}</span>,
    },
    {
      key: 'key',
      header: ti('colKey'),
      render: (row) => (row.api_key_saved ? ti('keySaved') : ti('keyNone')),
    },
    {
      key: 'test',
      header: ti('colLastTest'),
      render: (row) => <LastTest provider={row} running={testRunning && tested?.id === row.id} />,
    },
    {
      key: 'models',
      header: ti('colModels'),
      render: (row) =>
        row.models.length === 1
          ? ti('modelCountOne')
          : ti('modelCount', { count: row.models.length }),
    },
    {
      key: 'actions',
      header: ti('colActions'),
      render: (row) => (
        <div className={adminStyles.actions}>
          <button
            type="button"
            className={adminStyles.actionButton}
            aria-label={ti('actionTestFor', { name: row.name })}
            disabled={testRunning && tested?.id === row.id}
            data-testid="provider-test-button"
            onClick={() => {
              setTest((current) => ({
                providerId: row.id,
                request: current?.providerId === row.id ? current.request + 1 : 1,
              }));
            }}
          >
            {ti('actionTest')}
          </button>
          <button
            type="button"
            className={adminStyles.actionButton}
            aria-label={ti('actionEditFor', { name: row.name })}
            data-testid="provider-edit-button"
            onClick={() => {
              openDialog(row);
            }}
          >
            {ti('actionEdit')}
          </button>
          <button
            type="button"
            className={cx(adminStyles.actionButton, adminStyles.actionButtonDanger)}
            aria-label={ti('actionRemoveFor', { name: row.name })}
            data-testid="provider-remove-button"
            onClick={() => {
              setRemoving(row);
            }}
          >
            {ti('actionRemove')}
          </button>
        </div>
      ),
    },
  ];

  const loadError = providers.error ?? defaults.error;

  return (
    <>
      <div className={adminStyles.header}>
        <h1>{ti('providersTitle')}</h1>
        <button
          type="button"
          className={shared.button}
          data-testid="provider-add"
          onClick={() => {
            openDialog(null);
          }}
        >
          {ti('providersAdd')}
        </button>
      </div>
      <p className={shared.muted}>{ti('providersIntro')}</p>
      {providers.isPending || defaults.isPending ? (
        loadError ? null : (
          <p role="status" data-testid="providers-loading">
            {t('loading')}
          </p>
        )
      ) : null}
      {loadError ? (
        <div
          role="alert"
          className={cx(shared.banner, shared.bannerError)}
          data-testid="providers-load-error"
        >
          <p>{ti('providersLoadError')}</p>
          <p>{errorMessage(loadError)}</p>
          <button
            type="button"
            className={shared.button}
            onClick={() => {
              void providers.refetch();
              void defaults.refetch();
            }}
          >
            {t('retry')}
          </button>
        </div>
      ) : null}
      {providers.data !== undefined && defaults.data !== undefined ? (
        <>
          <section className={styles.section} aria-label={ti('providersTableCaption')}>
            <ResponsiveTable
              caption={ti('providersTableCaption')}
              columns={columns}
              rows={providers.data}
              rowKey={(row) => row.id}
              emptyState={ti('providersEmpty')}
              testId="providers-table"
            />
          </section>
          {tested === undefined || test === null ? null : (
            <ProviderTestPanel
              key={tested.id}
              provider={tested}
              request={test.request}
              onRunningChange={onRunningChange}
            />
          )}
          <DefaultModelsCard
            key={`${defaults.data.orchestrator?.model.id ?? ''}|${defaults.data.agents?.model.id ?? ''}`}
            providers={providers.data}
            defaults={defaults.data}
          />
        </>
      ) : null}
      <ProviderDialog
        key={dialog.instance}
        provider={dialog.provider}
        open={dialog.open}
        onClose={() => {
          setDialog((current) => ({ ...current, open: false }));
        }}
      />
      <RemoveProviderDialog
        provider={removing}
        onClose={() => {
          setRemoving(null);
          setTest((current) => (current?.providerId === removing?.id ? null : current));
        }}
      />
    </>
  );
}
