import { ME_ADMIN, mockApi } from '../../../../test/mockApi';
import type { MockApi, MockHandler } from '../../../../test/mockApi';
import type { TestRun } from '../../../../api/useTestRun';

type ProbeResult = TestRun['results'][number];

export const PROVIDER_ID = '019b0000-0000-7000-8000-00000000a001';
export const MODEL_GOOD = '019b0000-0000-7000-8000-00000000b001';
export const MODEL_WEAK = '019b0000-0000-7000-8000-00000000b002';
export const RUN_ID = '019b0000-0000-7000-8000-00000000c001';

export const GOOD_MODEL = {
  id: MODEL_GOOD,
  provider_id: PROVIDER_ID,
  name: 'qwen3-coder-30b',
  label: 'Qwen coder',
  tool_calling: 'supported',
  detected_tool_calling: 'supported',
  declared_tool_calling: null,
  context_tokens: 32768,
  detected_context_tokens: 32768,
  declared_context_tokens: null,
  warnings: [],
  disagrees_with_detection: false,
};

export const WEAK_MODEL = {
  id: MODEL_WEAK,
  provider_id: PROVIDER_ID,
  name: 'tiny-1b',
  label: null,
  tool_calling: 'unknown',
  detected_tool_calling: 'unknown',
  declared_tool_calling: null,
  context_tokens: 4096,
  detected_context_tokens: 4096,
  declared_context_tokens: null,
  warnings: [
    { code: 'tool_calling_unknown' },
    { code: 'context_too_small', have: 4096, need: 8000 },
  ],
  disagrees_with_detection: false,
};

export function finishedRun(target: string, results: ProbeResult[], status = 'succeeded'): TestRun {
  return {
    id: RUN_ID,
    target,
    status,
    requested_at: '2026-11-17T01:00:00Z',
    started_at: '2026-11-17T01:00:01Z',
    finished_at: '2026-11-17T01:00:05Z',
    failure: null,
    results,
  };
}

export function queuedRun(target: string): TestRun {
  return { ...finishedRun(target, []), status: 'queued', started_at: null, finished_at: null };
}

export const OK_DETAILS = { repositories: [] };

export const PROVIDER_RUN = finishedRun(`provider:${PROVIDER_ID}`, [
  { subject: 'provider', status: 'ok', message: 'The provider answered.', details: OK_DETAILS },
  {
    subject: 'model',
    model_id: MODEL_GOOD,
    status: 'ok',
    message: 'The model answered.',
    details: { ...OK_DETAILS, tool_calling: 'supported', context_tokens: 32768 },
  },
  {
    subject: 'model',
    model_id: MODEL_WEAK,
    status: 'ok',
    message: 'The model answered.',
    details: { ...OK_DETAILS, tool_calling: 'unknown', context_tokens: 4096 },
  },
]);

export const PROVIDER = {
  id: PROVIDER_ID,
  kind: 'openai_compatible',
  name: 'Colibri',
  base_url: 'http://localhost:11434/v1',
  api_key_saved: true,
  api_key_saved_at: '2026-11-16T22:00:00Z',
  created_at: '2026-11-16T22:00:00Z',
  updated_at: '2026-11-16T22:00:00Z',
  active_test_id: null,
  last_test: null,
  models: [GOOD_MODEL, WEAK_MODEL],
};

export const NO_DEFAULTS = { orchestrator: null, agents: null };

export const SECRET = 'sk-test-0123456789-secret';

export function validation(field: string, message: string) {
  return {
    status: 422,
    body: {
      error: {
        code: 'validation',
        message: 'Check the highlighted fields.',
        fields: [{ field, message }],
      },
    },
  };
}

/** An admin signed in, with whatever integrations endpoints the test serves. */
export function integrationsApi(extra: Record<string, MockHandler> = {}): MockApi {
  return mockApi({ 'GET /api/auth/me': { body: ME_ADMIN }, ...extra });
}
