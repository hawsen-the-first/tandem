import { QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { call, client } from '../../api/client';
import { createQueryClient } from '../../api/queryClient';
import { TEST_POLLING, useTestRun } from '../../api/useTestRun';
import { CapabilityWarning } from '../integrations/CapabilityWarning';
import { HostListEditor } from '../integrations/HostListEditor';
import { SecretField } from '../integrations/SecretField';
import { StatusBadge } from '../integrations/StatusBadge';
import { TestResultView, runStatus } from '../integrations/TestResultView';
import { UrlField } from '../integrations/UrlField';
import { checkHosts, checkOutboundUrl } from '../integrations/outboundRules';
import { mockApi } from '../../test/mockApi';
import {
  PROVIDER_RUN,
  RUN_ID,
  finishedRun,
  queuedRun,
} from '../../pages/admin/integrations/__tests__/fixtures';

describe('checkOutboundUrl (BR-19)', () => {
  it('accepts https anywhere and http only for local and private hosts', () => {
    expect(checkOutboundUrl('https://api.example.com/v1')).toBeNull();
    expect(checkOutboundUrl('http://localhost:11434/v1')).toBeNull();
    expect(checkOutboundUrl('http://127.0.0.1:8080')).toBeNull();
    expect(checkOutboundUrl('http://192.168.1.20/v1')).toBeNull();
    expect(checkOutboundUrl('http://10.0.0.5')).toBeNull();
    expect(checkOutboundUrl('http://ollama')).toBeNull();
    expect(checkOutboundUrl('http://gpu.local:8000')).toBeNull();
    expect(checkOutboundUrl('http://models.internal')).toBeNull();
  });

  it('gives the server reason for each refusal', () => {
    expect(checkOutboundUrl('')).toBe('Enter a URL');
    expect(checkOutboundUrl('api.example.com')).toBe('Use an http:// or https:// URL');
    expect(checkOutboundUrl('ftp://example.com')).toBe('Use an http:// or https:// URL');
    expect(checkOutboundUrl('https://user:pw@example.com')).toBe(
      'Do not put a user name or password in the URL',
    );
    expect(checkOutboundUrl('https://example.com/v1?key=1')).toBe(
      'Remove everything after ? or # from the URL',
    );
    expect(checkOutboundUrl('https://example.com/#x')).toContain('after ? or #');
    expect(checkOutboundUrl('http://api.example.com')).toContain('Use https:// for public servers');
    expect(checkOutboundUrl('http://8.8.8.8')).toContain('Use https://');
    expect(checkOutboundUrl('http://169.254.169.254/latest')).toContain('always blocked');
    expect(checkOutboundUrl('https://[fe80::1]/')).toContain('always blocked');
    expect(checkOutboundUrl('https://[fd00:ec2::254]/')).toContain('always blocked');
  });
});

describe('checkHosts (BR-18.4)', () => {
  it('accepts host names with an optional leading wildcard', () => {
    expect(checkHosts(['api.example.com', '*.github.com', 'pypi.org'])).toEqual({});
  });
  it('flags bad and duplicate rows by index', () => {
    const problems = checkHosts([
      'https://x.com',
      'ok.example',
      'OK.example',
      '',
      'a.*.b',
      'x'.repeat(254),
    ]);
    expect(problems[0]).toContain('host name');
    expect(problems[1]).toBeUndefined();
    expect(problems[2]).toBe('This host is already in the list');
    expect(problems[3]).toContain('host name');
    expect(problems[4]).toContain('host name');
    expect(problems[5]).toContain('host name');
  });
});

describe('StatusBadge', () => {
  it('always says the status in words, with the icon hidden', () => {
    render(
      <>
        <StatusBadge status="ok" />
        <StatusBadge status="error" />
        <StatusBadge status="warning" label="Untested" />
      </>,
    );
    expect(screen.getByText('OK')).toBeInTheDocument();
    expect(screen.getByText('Failed')).toBeInTheDocument();
    expect(screen.getByText('Untested')).toBeInTheDocument();
  });
});

function SecretHarness({ saved, onRemove }: { saved: boolean; onRemove?: boolean }) {
  const [value, setValue] = useState('');
  const [pending, setPending] = useState(false);
  return (
    <SecretField
      label="API key"
      value={value}
      onChange={setValue}
      saved={saved}
      savedAt="2026-11-16T22:00:00Z"
      removal={onRemove ? { pending, onChange: setPending } : undefined}
      testId="key"
    />
  );
}

describe('SecretField (BR-20)', () => {
  it('says a key is saved and when, never shows it, and explains that empty keeps it', async () => {
    const { container } = render(<SecretHarness saved onRemove />);
    const input = screen.getByTestId('key-input');
    expect(input).toHaveAttribute('type', 'password');
    expect(input).toHaveValue('');
    expect(input).toHaveAccessibleDescription(/Saved on .*Leave empty to keep the saved value/);
    expect(await axe(container)).toHaveNoViolations();
  });

  it('marks the saved key for removal and can undo it', async () => {
    const user = userEvent.setup();
    render(<SecretHarness saved onRemove />);
    await user.type(screen.getByTestId('key-input'), 'abc');
    await user.click(screen.getByTestId('key-remove'));
    expect(screen.getByTestId('key-status')).toHaveTextContent('Will be removed when you save');
    expect(screen.getByTestId('key-input')).toBeDisabled();
    expect(screen.getByTestId('key-input')).toHaveValue('');
    expect(screen.getByTestId('key-remove')).toHaveAttribute('aria-pressed', 'true');
    await user.click(screen.getByTestId('key-remove'));
    expect(screen.getByTestId('key-input')).toBeEnabled();
  });

  it('offers no remove button when nothing is saved', () => {
    render(<SecretHarness saved={false} onRemove />);
    expect(screen.getByTestId('key-status')).toHaveTextContent('Not saved');
    expect(screen.queryByTestId('key-remove')).toBeNull();
  });
});

describe('UrlField', () => {
  it('links its hint and the BR-19 reason to the input', async () => {
    const { container } = render(
      <UrlField
        label="Base URL"
        value="http://api.example.com"
        onChange={() => undefined}
        error={checkOutboundUrl('http://api.example.com') ?? undefined}
        testId="url"
      />,
    );
    const input = screen.getByTestId('url-input');
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription(
      /Include any path.*Use https:\/\/ for public servers/,
    );
    expect(await axe(container)).toHaveNoViolations();
  });
});

function HostHarness({ initial }: { initial: string[] }) {
  const [hosts, setHosts] = useState(initial);
  return (
    <HostListEditor hosts={hosts} onChange={setHosts} errors={checkHosts(hosts)} testId="hosts" />
  );
}

describe('HostListEditor', () => {
  it('adds, edits and removes rows, each with its own name and error', async () => {
    const user = userEvent.setup();
    const { container } = render(<HostHarness initial={['api.example.com']} />);
    await user.click(screen.getByTestId('hosts-add'));
    const second = screen.getByRole('textbox', { name: 'Host 2' });
    await user.type(second, 'not a host');
    expect(second).toHaveAttribute('aria-invalid', 'true');
    expect(second).toHaveAccessibleDescription(/Enter a host name/);
    expect(await axe(container)).toHaveNoViolations();
    await user.click(screen.getByRole('button', { name: 'Remove host 2' }));
    expect(screen.queryByRole('textbox', { name: 'Host 2' })).toBeNull();
    expect(screen.getByRole('textbox', { name: 'Host 1' })).toHaveValue('api.example.com');
  });
});

describe('CapabilityWarning', () => {
  it('explains each missing capability and that jobs will refuse to run', () => {
    render(
      <CapabilityWarning
        warnings={[
          { code: 'tool_calling_unknown' },
          { code: 'context_too_small', have: 4096, need: 8000 },
          { code: 'tool_calling_unsupported' },
          { code: 'context_unknown' },
        ]}
      />,
    );
    expect(screen.getByText(/has not shown tool calling/)).toBeInTheDocument();
    expect(
      screen.getByText(/4,096 tokens of context; jobs need at least 8,000/),
    ).toBeInTheDocument();
    expect(screen.getByText(/does not support tool calling/)).toBeInTheDocument();
    expect(screen.getByText(/context length of this model is not known/)).toBeInTheDocument();
  });
  it('renders nothing for a suitable model', () => {
    const { container } = render(<CapabilityWarning warnings={[]} />);
    expect(container).toBeEmptyDOMElement();
  });
});

describe('TestResultView', () => {
  it('summarises a run in a polite live region and lists each result', async () => {
    const { container } = render(<TestResultView run={PROVIDER_RUN} running={false} testId="r" />);
    const region = screen.getByRole('status');
    expect(region).toHaveAttribute('aria-live', 'polite');
    expect(screen.getByTestId('r-status')).toHaveAttribute('data-status', 'warning');
    expect(screen.getAllByTestId('r-item')).toHaveLength(3);
    expect(screen.getByText('Tool calling: Supported')).toBeInTheDocument();
    expect(screen.getByText('Context: 32,768 tokens')).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows progress, a whole-run failure and the never-tested state', () => {
    const { rerender } = render(<TestResultView run={undefined} running testId="r" />);
    expect(screen.getByTestId('r-summary')).toHaveTextContent('Waiting for a worker');
    rerender(
      <TestResultView
        run={{
          ...finishedRun('runtime', [], 'failed'),
          failure: { code: 'no_worker_available', message: 'No worker is available.' },
        }}
        running={false}
        testId="r"
      />,
    );
    expect(screen.getByTestId('r-status')).toHaveAttribute('data-status', 'error');
    expect(screen.getByTestId('r-failure')).toHaveTextContent('No worker is available.');
    rerender(<TestResultView run={null} running={false} testId="r" />);
    expect(screen.getByTestId('r-summary')).toHaveTextContent('Not tested yet');
  });

  it('lists GitHub repositories with the total when capped', () => {
    const run = finishedRun('connector:github', [
      {
        subject: 'connector',
        status: 'ok',
        message: 'The token works.',
        details: {
          account_login: 'octo',
          repositories: ['octo/a', 'octo/b'],
          repository_total: 140,
          token_expires_at: '2027-01-01T00:00:00Z',
        },
      },
    ]);
    render(<TestResultView run={run} running={false} testId="r" />);
    expect(screen.getByText('Account: octo')).toBeInTheDocument();
    expect(screen.getByText('Repositories it can reach (140)')).toBeInTheDocument();
    expect(screen.getByText('Showing the first 2 of 140')).toBeInTheDocument();
    expect(screen.getByText(/^Expires /)).toBeInTheDocument();
  });

  it('rates a run by its results', () => {
    expect(runStatus(null)).toBe('unknown');
    expect(runStatus(queuedRun('runtime'))).toBe('running');
    expect(
      runStatus(
        finishedRun('search:brave', [
          {
            subject: 'search',
            status: 'unauthorised',
            message: 'x',
            details: { repositories: [] },
          },
        ]),
      ),
    ).toBe('error');
    expect(
      runStatus(
        finishedRun('search:brave', [
          { subject: 'search', status: 'ok', message: 'x', details: { repositories: [] } },
        ]),
      ),
    ).toBe('ok');
  });
});

const START_PATH = '/api/admin/connectors/github/test';

function RunHarness({ activeRunId }: { activeRunId?: string | undefined }) {
  const test = useTestRun({
    start: () => call(client.POST(START_PATH)),
    activeRunId,
  });
  return (
    <>
      <button type="button" onClick={test.begin}>
        start
      </button>
      <TestResultView
        run={test.run}
        running={test.running}
        timedOut={test.timedOut}
        error={test.error}
        testId="run"
      />
    </>
  );
}

function renderRun(activeRunId?: string) {
  return render(
    <QueryClientProvider client={createQueryClient()}>
      <RunHarness activeRunId={activeRunId} />
    </QueryClientProvider>,
  );
}

describe('useTestRun', () => {
  const saved = { ...TEST_POLLING };
  afterEach(() => {
    Object.assign(TEST_POLLING, saved);
  });

  it('starts a run and polls until it finishes', async () => {
    TEST_POLLING.intervalMs = 20;
    let polls = 0;
    const api = mockApi({
      [`POST ${START_PATH}`]: { status: 202, body: queuedRun('connector:github') },
      [`GET /api/admin/test-runs/${RUN_ID}`]: () => {
        polls += 1;
        return {
          body:
            polls < 3
              ? queuedRun('connector:github')
              : finishedRun('connector:github', [
                  {
                    subject: 'connector',
                    status: 'ok',
                    message: 'The token works.',
                    details: { repositories: [] },
                  },
                ]),
        };
      },
    });
    const user = userEvent.setup();
    renderRun();
    await user.click(screen.getByRole('button', { name: 'start' }));
    expect(await screen.findByText('The token works.')).toBeInTheDocument();
    expect(screen.getByTestId('run-status')).toHaveAttribute('data-status', 'ok');
    expect(polls).toBe(3);
    const after = api.calls.length;
    await act(() => new Promise((resolve) => setTimeout(resolve, 80)));
    expect(api.calls.length).toBe(after);
  });

  it('follows the run already going when a start answers 409', async () => {
    mockApi({
      [`POST ${START_PATH}`]: {
        status: 409,
        body: {
          error: { code: 'already_running', message: 'A test is already running.', run_id: RUN_ID },
        },
      },
      [`GET /api/admin/test-runs/${RUN_ID}`]: {
        body: finishedRun('connector:github', [
          {
            subject: 'connector',
            status: 'unauthorised',
            message: 'GitHub refused the token.',
            details: { repositories: [] },
          },
        ]),
      },
    });
    const user = userEvent.setup();
    renderRun();
    await user.click(screen.getByRole('button', { name: 'start' }));
    expect(await screen.findByText('GitHub refused the token.')).toBeInTheDocument();
  });

  it('follows a run that was already active when the page loaded', async () => {
    mockApi({
      [`GET /api/admin/test-runs/${RUN_ID}`]: {
        body: finishedRun('connector:github', [
          {
            subject: 'connector',
            status: 'ok',
            message: 'The token works.',
            details: { repositories: [] },
          },
        ]),
      },
    });
    renderRun(RUN_ID);
    expect(await screen.findByText('The token works.')).toBeInTheDocument();
  });

  it('stops polling after the limit and says the test keeps running', async () => {
    TEST_POLLING.intervalMs = 10;
    TEST_POLLING.limitMs = 60;
    mockApi({
      [`POST ${START_PATH}`]: { status: 202, body: queuedRun('connector:github') },
      [`GET /api/admin/test-runs/${RUN_ID}`]: { body: queuedRun('connector:github') },
    });
    const user = userEvent.setup();
    renderRun();
    await user.click(screen.getByRole('button', { name: 'start' }));
    await waitFor(() => {
      expect(screen.getByTestId('run-summary')).toHaveTextContent('taking longer than expected');
    });
  });

  it('reports a refused start such as the rate limit', async () => {
    mockApi({
      [`POST ${START_PATH}`]: {
        status: 429,
        headers: { 'retry-after': '30' },
        body: { error: { code: 'rate_limited', message: 'Too many requests.' } },
      },
    });
    const user = userEvent.setup();
    renderRun();
    await user.click(screen.getByRole('button', { name: 'start' }));
    expect(
      await screen.findByText('Too many attempts. Try again in 30 seconds'),
    ).toBeInTheDocument();
    expect(screen.getByTestId('run-status')).toHaveAttribute('data-status', 'error');
  });
});
