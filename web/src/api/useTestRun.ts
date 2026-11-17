import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import { call, client } from './client';
import { ApiError } from './errors';
import type { components } from './schema';

export type TestRun = components['schemas']['TestRunDto'];

/**
 * How often a running test is polled and for how long (NFR Design pattern 2: 1 s for at most 90 s).
 * Tests may shorten these.
 */
export const TEST_POLLING = { intervalMs: 1000, limitMs: 90_000 };

export function isFinished(run: TestRun | undefined): boolean {
  return run !== undefined && (run.status === 'succeeded' || run.status === 'failed');
}

interface Options {
  /** Starts a run; resolves to the run the server queued (202). */
  start: () => Promise<TestRun>;
  /** A run already queued or running when the page loaded (`active_test_id`). */
  activeRunId?: string | null | undefined;
  /** Called once when a run this hook follows finishes. */
  onFinished?: (run: TestRun) => void;
}

export interface TestRunState {
  /** The run being followed, with the results so far. */
  run: TestRun | undefined;
  /** A run is being started, is queued or is running, and polling has not given up. */
  running: boolean;
  /** Polling stopped after the limit while the run was still going. */
  timedOut: boolean;
  /** Starting failed (other than "already running", which is followed instead). */
  error: unknown;
  begin: () => void;
}

/**
 * Start a background test and follow it until it finishes. A start refused because a run is
 * already going follows that run instead.
 */
export function useTestRun({ start, activeRunId, onFinished }: Options): TestRunState {
  const [runId, setRunId] = useState<string | null>(activeRunId ?? null);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [timedOut, setTimedOut] = useState(false);
  const reported = useRef<string | null>(null);
  const finishedRef = useRef(onFinished);
  useEffect(() => {
    finishedRef.current = onFinished;
  });

  const query = useQuery({
    queryKey: ['admin', 'test-runs', runId],
    queryFn: () =>
      call(client.GET('/api/admin/test-runs/{id}', { params: { path: { id: runId ?? '' } } })),
    enabled: runId !== null && !timedOut,
    refetchInterval: (current) =>
      isFinished(current.state.data) ? false : TEST_POLLING.intervalMs,
    retry: false,
  });
  const run = runId === null ? undefined : query.data;
  const finished = isFinished(run);

  useEffect(() => {
    if (runId === null || finished) {
      return;
    }
    const timer = window.setTimeout(() => {
      setTimedOut(true);
    }, TEST_POLLING.limitMs);
    return () => {
      window.clearTimeout(timer);
    };
  }, [runId, finished]);

  useEffect(() => {
    if (run !== undefined && finished && reported.current !== run.id) {
      reported.current = run.id;
      finishedRef.current?.(run);
    }
  }, [run, finished]);

  const begin = useCallback(() => {
    setStarting(true);
    setError(null);
    setTimedOut(false);
    start()
      .then((queued) => {
        setRunId(queued.id);
      })
      .catch((failure: unknown) => {
        if (failure instanceof ApiError && failure.code === 'already_running' && failure.runId) {
          setRunId(failure.runId);
        } else {
          setError(failure);
        }
      })
      .finally(() => {
        setStarting(false);
      });
  }, [start]);

  const running = starting || (runId !== null && !finished && !timedOut && !query.isError);
  return { run, running, timedOut, error: error ?? (query.isError ? query.error : null), begin };
}
