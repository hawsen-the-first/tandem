import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useId, useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage, fieldErrors } from '../../../api/messages';
import { useTestRun } from '../../../api/useTestRun';
import { SecretField } from '../../../components/integrations/SecretField';
import { StatusBadge } from '../../../components/integrations/StatusBadge';
import { TestResultView } from '../../../components/integrations/TestResultView';
import { UrlField } from '../../../components/integrations/UrlField';
import { checkOutboundUrl } from '../../../components/integrations/outboundRules';
import { useToast } from '../../../components/Toast';
import { t } from '../../../i18n/messages';
import { ti } from '../../../i18n/messages.integrations';
import { cx } from '../../../styles/cx';
import shared from '../../../styles/shared.module.css';
import adminStyles from '../admin.module.css';
import type { SearchKind, SearchOverview } from './api';
import { ACTIVE_TEST_REFRESH_MS, SEARCH_KEY } from './api';
import styles from './integrations.module.css';

export function kindLabel(kind: string): string {
  switch (kind) {
    case 'searxng':
      return ti('searchKindSearxng');
    case 'brave':
      return ti('searchKindBrave');
    default:
      return ti('searchKindTavily');
  }
}

interface KindProblems {
  baseUrl?: string;
  apiKey?: string;
}

/** Client checks for one search kind's settings (BR-16.2); the server checks again. */
export function checkSearchKind(
  kind: Pick<SearchKind, 'needs_url' | 'needs_key' | 'api_key_saved'>,
  values: { baseUrl: string; apiKey: string; removeKey: boolean },
): KindProblems {
  const problems: KindProblems = {};
  if (kind.needs_url && values.baseUrl.trim() !== '') {
    const reason = checkOutboundUrl(values.baseUrl);
    if (reason !== null) {
      problems.baseUrl = reason;
    }
  }
  if (kind.needs_key && values.apiKey === '' && (!kind.api_key_saved || values.removeKey)) {
    problems.apiKey = ti('searchKeyRequired');
  }
  return problems;
}

function SearchKindCard({ kind }: { kind: SearchKind }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const headingId = useId();
  const label = kindLabel(kind.kind);
  const [baseUrl, setBaseUrl] = useState(kind.base_url ?? '');
  const [apiKey, setApiKey] = useState('');
  const [removeKey, setRemoveKey] = useState(false);
  const [problems, setProblems] = useState<KindProblems>({});
  const [failure, setFailure] = useState<string | null>(null);

  const save = useMutation({
    mutationFn: (body: { base_url?: string | null; api_key?: string; remove_api_key: boolean }) =>
      call(
        client.PUT('/api/admin/search/kinds/{kind}', {
          params: { path: { kind: kind.kind } },
          body,
        }),
      ),
    onSuccess: (overview) => {
      queryClient.setQueryData(SEARCH_KEY, overview);
      setRemoveKey(false);
      toast.show('success', ti('searchSaved', { kind: label }));
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      setProblems({
        ...(fields['base_url'] ? { baseUrl: fields['base_url'] } : {}),
        ...(fields['api_key'] ? { apiKey: fields['api_key'] } : {}),
      });
      setFailure(errorMessage(error));
    },
  });

  const activate = useMutation({
    mutationFn: () => call(client.PUT('/api/admin/search/active', { body: { kind: kind.kind } })),
    onSuccess: (result) => {
      void queryClient.invalidateQueries({ queryKey: SEARCH_KEY });
      toast.show(
        result.untested ? 'info' : 'success',
        result.untested
          ? ti('searchActivatedUntested', { kind: label })
          : ti('searchActivated', { kind: label }),
      );
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  const start = useCallback(
    () =>
      call(
        client.POST('/api/admin/search/kinds/{kind}/test', {
          params: { path: { kind: kind.kind } },
        }),
      ),
    [kind.kind],
  );
  const test = useTestRun({
    start,
    activeRunId: kind.active_test_id,
    onFinished: () => {
      void queryClient.invalidateQueries({ queryKey: SEARCH_KEY });
    },
  });

  return (
    <section
      className={shared.card}
      aria-labelledby={headingId}
      data-testid={`search-card-${kind.kind}`}
    >
      <div className={styles.cardHead}>
        <h2 id={headingId}>{label}</h2>
        {kind.active ? (
          <StatusBadge
            status="ok"
            label={ti('searchActive')}
            testId={`search-active-${kind.kind}`}
          />
        ) : kind.configured ? null : (
          <StatusBadge status="unknown" label={ti('searchNotConfigured')} />
        )}
      </div>
      <form
        className={styles.form}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          setFailure(null);
          const found = checkSearchKind(kind, { baseUrl, apiKey, removeKey });
          setProblems(found);
          if (found.baseUrl !== undefined || found.apiKey !== undefined) {
            return;
          }
          const body: { base_url?: string | null; api_key?: string; remove_api_key: boolean } = {
            remove_api_key: removeKey,
          };
          if (kind.needs_url) {
            body.base_url = baseUrl.trim() === '' ? null : baseUrl.trim();
          }
          if (kind.needs_key && !removeKey && apiKey !== '') {
            body.api_key = apiKey;
          }
          // The key leaves the browser with this request and is not kept (BR-20.3).
          setApiKey('');
          save.mutate(body);
        }}
      >
        {kind.needs_url ? (
          <UrlField
            label={ti('colBaseUrl')}
            hint={ti('searchSearxngHint')}
            value={baseUrl}
            error={problems.baseUrl}
            placeholder="https://search.example.com"
            testId={`search-url-${kind.kind}`}
            onChange={setBaseUrl}
          />
        ) : null}
        {kind.needs_key ? (
          <SecretField
            label={ti('providerKey')}
            value={apiKey}
            saved={kind.api_key_saved}
            savedAt={kind.api_key_saved_at}
            error={problems.apiKey}
            required={!kind.api_key_saved}
            removal={{ pending: removeKey, onChange: setRemoveKey }}
            testId={`search-key-${kind.kind}`}
            onChange={setApiKey}
          />
        ) : null}
        {failure === null ? null : (
          <p role="alert" className={cx(shared.banner, shared.bannerError)}>
            {failure}
          </p>
        )}
        <div className={styles.actionsRow}>
          <button
            type="submit"
            className={shared.button}
            aria-label={ti('searchSaveFor', { kind: label })}
            disabled={save.isPending}
            data-testid={`search-save-${kind.kind}`}
          >
            {ti('searchSave')}
          </button>
          <button
            type="button"
            className={cx(shared.button, shared.secondary)}
            aria-label={ti('searchTestFor', { kind: label })}
            disabled={!kind.configured || test.running}
            data-testid={`search-test-${kind.kind}`}
            onClick={test.begin}
          >
            {ti('searchTest')}
          </button>
          {kind.active ? null : (
            <button
              type="button"
              className={adminStyles.actionButton}
              aria-label={ti('searchMakeActiveFor', { kind: label })}
              disabled={!kind.configured || activate.isPending}
              data-testid={`search-activate-${kind.kind}`}
              onClick={() => {
                setFailure(null);
                activate.mutate();
              }}
            >
              {ti('searchMakeActive')}
            </button>
          )}
        </div>
      </form>
      <TestResultView
        run={test.run ?? (test.running ? undefined : kind.last_test)}
        running={test.running}
        timedOut={test.timedOut}
        error={test.error}
        heading={ti('searchTestFor', { kind: label })}
        labelFor={() => label}
        testId={`search-result-${kind.kind}`}
      />
    </section>
  );
}

function Overview({ overview }: { overview: SearchOverview }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [failure, setFailure] = useState<string | null>(null);
  const turnOff = useMutation({
    mutationFn: () => call(client.PUT('/api/admin/search/active', { body: { kind: null } })),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: SEARCH_KEY });
      toast.show('success', ti('searchTurnedOff'));
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });
  const active = overview.active ?? null;
  return (
    <>
      <div
        className={cx(shared.banner, active === null && shared.bannerWarning, styles.section)}
        data-testid="search-state"
      >
        <div className={styles.actionsRow}>
          <span>
            {active === null
              ? ti('searchNoneActive')
              : ti('searchActiveIs', { kind: kindLabel(active) })}
          </span>
          {active === null ? null : (
            <button
              type="button"
              className={adminStyles.actionButton}
              disabled={turnOff.isPending}
              data-testid="search-turn-off"
              onClick={() => {
                setFailure(null);
                turnOff.mutate();
              }}
            >
              {ti('searchTurnOff')}
            </button>
          )}
        </div>
        {failure === null ? null : (
          <p role="alert" className={shared.bannerError}>
            {failure}
          </p>
        )}
      </div>
      <div className={styles.cards}>
        {overview.kinds.map((kind) => (
          <SearchKindCard key={kind.kind} kind={kind} />
        ))}
      </div>
    </>
  );
}

export function Component() {
  const search = useQuery({
    queryKey: SEARCH_KEY,
    queryFn: () => call(client.GET('/api/admin/search')),
    refetchInterval: (query) =>
      query.state.data?.kinds.some((kind) => kind.active_test_id) ? ACTIVE_TEST_REFRESH_MS : false,
  });
  return (
    <>
      <h1>{ti('searchTitle')}</h1>
      <p className={shared.muted}>{ti('searchIntro')}</p>
      {search.isPending ? (
        <p role="status" data-testid="search-loading">
          {t('loading')}
        </p>
      ) : search.isError ? (
        <div
          role="alert"
          className={cx(shared.banner, shared.bannerError)}
          data-testid="search-load-error"
        >
          <p>{ti('searchLoadError')}</p>
          <p>{errorMessage(search.error)}</p>
          <button
            type="button"
            className={shared.button}
            onClick={() => {
              void search.refetch();
            }}
          >
            {t('retry')}
          </button>
        </div>
      ) : (
        <Overview overview={search.data} />
      )}
    </>
  );
}
