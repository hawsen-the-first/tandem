import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useId, useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage, fieldErrors } from '../../../api/messages';
import { useTestRun } from '../../../api/useTestRun';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { SecretField } from '../../../components/integrations/SecretField';
import { TestResultView } from '../../../components/integrations/TestResultView';
import { useToast } from '../../../components/Toast';
import { t } from '../../../i18n/messages';
import { ti } from '../../../i18n/messages.integrations';
import { cx } from '../../../styles/cx';
import shared from '../../../styles/shared.module.css';
import adminStyles from '../admin.module.css';
import type { Connector } from './api';
import { ACTIVE_TEST_REFRESH_MS, GITHUB_KEY } from './api';
import styles from './integrations.module.css';

function PermissionsNotice() {
  const headingId = useId();
  return (
    <section
      className={cx(shared.banner, styles.section)}
      aria-labelledby={headingId}
      data-testid="github-permissions"
    >
      <h2 id={headingId}>{ti('githubPermissionsTitle')}</h2>
      <p>{ti('githubPermissions')}</p>
      <p className={shared.muted}>{ti('githubPermissionsAdvice')}</p>
    </section>
  );
}

function ConnectorView({ connector }: { connector: Connector }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [token, setToken] = useState('');
  const [problem, setProblem] = useState<string | undefined>(undefined);
  const [failure, setFailure] = useState<string | null>(null);
  const [confirmRemove, setConfirmRemove] = useState(false);

  const save = useMutation({
    mutationFn: (value: string) =>
      call(client.PUT('/api/admin/connectors/github', { body: { token: value } })),
    onSuccess: (saved) => {
      queryClient.setQueryData(GITHUB_KEY, saved);
      toast.show('success', ti('githubSaved'));
    },
    onError: (error) => {
      setProblem(fieldErrors(error)['token']);
      setFailure(errorMessage(error));
    },
  });

  const remove = useMutation({
    mutationFn: () => call(client.DELETE('/api/admin/connectors/github')),
    onSuccess: () => {
      setConfirmRemove(false);
      void queryClient.invalidateQueries({ queryKey: GITHUB_KEY });
      toast.show('success', ti('githubRemoved'));
    },
    onError: (error) => {
      setConfirmRemove(false);
      setFailure(errorMessage(error));
    },
  });

  const start = useCallback(() => call(client.POST('/api/admin/connectors/github/test')), []);
  const test = useTestRun({
    start,
    activeRunId: connector.active_test_id,
    onFinished: () => {
      void queryClient.invalidateQueries({ queryKey: GITHUB_KEY });
    },
  });

  return (
    <>
      <p
        className={cx(shared.banner, !connector.configured && shared.bannerWarning)}
        data-testid="github-status"
      >
        {connector.configured
          ? ti('githubStatusSaved', {
              date: connector.saved_at ? new Date(connector.saved_at).toLocaleString() : '',
            })
          : ti('githubStatusNone')}
        {connector.account_login ? (
          <> · {ti('githubAccount', { login: connector.account_login })}</>
        ) : null}
        {connector.token_expires_at ? (
          <>
            {' '}
            · {ti('githubExpires', { date: new Date(connector.token_expires_at).toLocaleString() })}
          </>
        ) : null}
      </p>
      <PermissionsNotice />
      <form
        className={cx(styles.form, styles.formNarrow, styles.section)}
        noValidate
        data-testid="github-form"
        onSubmit={(event) => {
          event.preventDefault();
          setFailure(null);
          if (token.trim() === '') {
            setProblem(ti('githubTokenRequired'));
            return;
          }
          setProblem(undefined);
          const value = token;
          // The token leaves the browser with this request and is not kept (BR-20.3).
          setToken('');
          save.mutate(value);
        }}
      >
        <SecretField
          label={ti('githubToken')}
          value={token}
          saved={connector.configured}
          savedAt={connector.saved_at}
          error={problem}
          required={!connector.configured}
          testId="github-token"
          onChange={setToken}
        />
        {failure === null ? null : (
          <p
            role="alert"
            className={cx(shared.banner, shared.bannerError)}
            data-testid="github-error"
          >
            {failure}
          </p>
        )}
        <div className={styles.actionsRow}>
          <button
            type="submit"
            className={shared.button}
            disabled={save.isPending}
            data-testid="github-save"
          >
            {ti('githubSave')}
          </button>
          <button
            type="button"
            className={cx(shared.button, shared.secondary)}
            disabled={!connector.configured || test.running}
            data-testid="github-test"
            onClick={test.begin}
          >
            {ti('githubTest')}
          </button>
          {connector.configured ? (
            <button
              type="button"
              className={cx(adminStyles.actionButton, adminStyles.actionButtonDanger)}
              data-testid="github-remove"
              onClick={() => {
                setConfirmRemove(true);
              }}
            >
              {ti('githubRemove')}
            </button>
          ) : null}
        </div>
      </form>
      <TestResultView
        run={test.run ?? (test.running ? undefined : connector.last_test)}
        running={test.running}
        timedOut={test.timedOut}
        error={test.error}
        heading={ti('githubTest')}
        labelFor={() => ti('navGitHub')}
        testId="github-result"
      />
      <ConfirmDialog
        open={confirmRemove}
        title={ti('githubRemoveTitle')}
        confirmLabel={ti('githubRemove')}
        destructive
        busy={remove.isPending}
        testId="github-remove-dialog"
        onConfirm={() => {
          remove.mutate();
        }}
        onCancel={() => {
          setConfirmRemove(false);
        }}
      >
        <p>{ti('githubRemoveBody')}</p>
      </ConfirmDialog>
    </>
  );
}

export function Component() {
  const connector = useQuery({
    queryKey: GITHUB_KEY,
    queryFn: () => call(client.GET('/api/admin/connectors/github')),
    refetchInterval: (query) => (query.state.data?.active_test_id ? ACTIVE_TEST_REFRESH_MS : false),
  });
  return (
    <>
      <h1>{ti('githubTitle')}</h1>
      <p className={shared.muted}>{ti('githubIntro')}</p>
      {connector.isPending ? (
        <p role="status" data-testid="github-loading">
          {t('loading')}
        </p>
      ) : connector.isError ? (
        <div
          role="alert"
          className={cx(shared.banner, shared.bannerError)}
          data-testid="github-load-error"
        >
          <p>{ti('githubLoadError')}</p>
          <p>{errorMessage(connector.error)}</p>
          <button
            type="button"
            className={shared.button}
            onClick={() => {
              void connector.refetch();
            }}
          >
            {t('retry')}
          </button>
        </div>
      ) : (
        <ConnectorView connector={connector.data} />
      )}
    </>
  );
}
