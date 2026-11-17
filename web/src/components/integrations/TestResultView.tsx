import type { components } from '../../api/schema';
import type { TestRun } from '../../api/useTestRun';
import { errorMessage } from '../../api/messages';
import { ti } from '../../i18n/messages.integrations';
import type { BadgeStatus } from './StatusBadge';
import { StatusBadge } from './StatusBadge';
import styles from './integrations.module.css';

type ProbeResult = components['schemas']['ProbeResultDto'];

/** "Supported", "Not supported" or "Unknown" for a tool-calling value. */
export function toolLabel(value: string | null | undefined): string {
  switch (value) {
    case 'supported':
      return ti('toolSupported');
    case 'unsupported':
      return ti('toolUnsupported');
    default:
      return ti('toolUnknown');
  }
}

export function resultStatus(result: ProbeResult): BadgeStatus {
  if (result.status === 'ok') {
    return result.subject === 'model' && result.details.tool_calling !== 'supported'
      ? 'warning'
      : 'ok';
  }
  return result.status === 'not_configured' ? 'unknown' : 'error';
}

/** One status for a whole run: running, failed, partly working or working. */
export function runStatus(run: TestRun | null | undefined): BadgeStatus {
  if (run === null || run === undefined) {
    return 'unknown';
  }
  if (run.status === 'queued' || run.status === 'running') {
    return 'running';
  }
  if (run.status === 'failed' || run.results.length === 0) {
    return 'error';
  }
  const statuses = run.results.map(resultStatus);
  if (statuses.every((status) => status === 'error')) {
    return 'error';
  }
  return statuses.every((status) => status === 'ok') ? 'ok' : 'warning';
}

function defaultLabel(result: ProbeResult): string {
  switch (result.subject) {
    case 'provider':
      return ti('resultProvider');
    case 'search':
      return ti('navSearch');
    case 'connector':
      return ti('navGitHub');
    case 'runtime':
      return ti('sandboxTierContainer');
    default:
      return ti('resultModel', { name: result.model_id ?? '' });
  }
}

function Details({ result }: { result: ProbeResult }) {
  const { details } = result;
  const total = details.repository_total ?? details.repositories.length;
  return (
    <>
      {details.tool_calling ? (
        <p className={styles.resultDetail}>
          {ti('resultTools', { value: toolLabel(details.tool_calling) })}
        </p>
      ) : null}
      {details.context_tokens !== null && details.context_tokens !== undefined ? (
        <p className={styles.resultDetail}>
          {ti('resultContext', { value: details.context_tokens.toLocaleString() })}
        </p>
      ) : null}
      {details.account_login ? (
        <p className={styles.resultDetail}>
          {ti('githubAccount', { login: details.account_login })}
        </p>
      ) : null}
      {details.token_expires_at ? (
        <p className={styles.resultDetail}>
          {ti('githubExpires', { date: new Date(details.token_expires_at).toLocaleString() })}
        </p>
      ) : null}
      {details.runtime_name ? (
        <p className={styles.resultDetail}>
          {ti('sandboxRuntimeOk', {
            name: details.runtime_name,
            version: details.runtime_version ?? '',
          })}
        </p>
      ) : null}
      {result.note === 'no_results' ? (
        <p className={styles.resultDetail}>{ti('searchNoResultsNote')}</p>
      ) : null}
      {details.repositories.length > 0 ? (
        <>
          <p className={styles.resultDetail}>{ti('githubRepos', { total })}</p>
          {total > details.repositories.length ? (
            <p className={styles.resultDetail}>
              {ti('githubReposShown', { shown: details.repositories.length, total })}
            </p>
          ) : null}
          <ul className={styles.repoList} data-testid="test-result-repositories">
            {details.repositories.map((name) => (
              <li key={name}>{name}</li>
            ))}
          </ul>
        </>
      ) : null}
    </>
  );
}

interface TestResultViewProps {
  /** The run being followed, or the last finished one. */
  run: TestRun | null | undefined;
  running: boolean;
  timedOut?: boolean | undefined;
  /** A failure to start or follow the run. */
  error?: unknown;
  heading?: string | undefined;
  /** How to name each result; defaults by subject. */
  labelFor?: ((result: ProbeResult) => string) | undefined;
  testId: string;
}

/**
 * A test's progress and results. The summary line is a polite live region, so the outcome is
 * announced when it arrives while the admin keeps reading.
 */
export function TestResultView({
  run,
  running,
  timedOut = false,
  error,
  heading,
  labelFor = defaultLabel,
  testId,
}: TestResultViewProps) {
  const status: BadgeStatus = running ? 'running' : runStatus(run);
  let summary: string;
  if (running) {
    summary = run?.status === 'running' ? ti('testing') : ti('testQueued');
  } else if (timedOut) {
    summary = ti('testTimedOut');
  } else if (error !== null && error !== undefined) {
    summary = errorMessage(error);
  } else if (run === null || run === undefined) {
    summary = ti('testNever');
  } else {
    summary = ti('testStartedBy', {
      time: new Date(run.finished_at ?? run.requested_at).toLocaleString(),
    });
  }
  const results = run?.results ?? [];
  return (
    <section
      className={styles.results}
      aria-label={heading ?? ti('testResultsHeading')}
      data-testid={testId}
    >
      <div role="status" aria-live="polite" className={styles.resultsSummary}>
        <StatusBadge status={error ? 'error' : status} testId={`${testId}-status`} />
        <span data-testid={`${testId}-summary`}>{summary}</span>
        {run?.failure ? <span data-testid={`${testId}-failure`}>{run.failure.message}</span> : null}
      </div>
      {results.length > 0 ? (
        <ul className={styles.resultList}>
          {results.map((result, index) => (
            <li
              key={`${result.subject}-${result.model_id ?? String(index)}`}
              className={styles.resultItem}
              data-testid={`${testId}-item`}
            >
              <div className={styles.resultHead}>
                <span>{labelFor(result)}</span>
                <StatusBadge status={resultStatus(result)} />
              </div>
              <p className={styles.resultMessage}>{result.message}</p>
              <Details result={result} />
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
