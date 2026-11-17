import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { ME_MEMBER, mockApi } from '../../../../test/mockApi';
import type { MockRequest } from '../../../../test/mockApi';
import { renderApp } from '../../../../test/render';
import { checkProvider } from '../providers/ProviderDialog';
import { newModelDraft } from '../providers/ModelListEditor';
import {
  GOOD_MODEL,
  MODEL_GOOD,
  MODEL_WEAK,
  NO_DEFAULTS,
  PROVIDER,
  PROVIDER_ID,
  PROVIDER_RUN,
  RUN_ID,
  SECRET,
  WEAK_MODEL,
  integrationsApi,
  queuedRun,
  validation,
} from './fixtures';

async function openPage(
  extra = {},
  providers: unknown[] = [PROVIDER],
  defaults: unknown = NO_DEFAULTS,
) {
  const api = integrationsApi({
    'GET /api/admin/providers': { body: providers },
    'GET /api/admin/model-defaults': { body: defaults },
    ...extra,
  });
  const view = renderApp('/admin/integrations/providers');
  await screen.findByRole('heading', { name: 'Model providers' });
  if (providers.length > 0) {
    await screen.findByTestId('providers-table');
  }
  return { api, ...view };
}

function lastCall(calls: MockRequest[], method: string, path: string): MockRequest | undefined {
  return calls.filter((call) => call.method === method && call.path === path).at(-1);
}

describe('checkProvider', () => {
  it('needs a name up to 60 characters, a valid URL and unique model names', () => {
    const model = { ...newModelDraft(), name: 'a' };
    const twin = { ...newModelDraft(), name: 'a' };
    const problems = checkProvider({
      name: ' ',
      baseUrl: 'http://api.example.com',
      models: [model, twin],
    });
    expect(problems.name).toBe('Enter a name up to 60 characters');
    expect(problems.baseUrl).toContain('Use https://');
    expect(problems.modelRows[twin.key]?.name).toBe('This model is already listed');
    expect(
      checkProvider({ name: 'x'.repeat(61), baseUrl: 'https://a.b', models: [] }).name,
    ).toBeDefined();
    const big = { ...newModelDraft(), name: 'm', declaredContext: '10000001' };
    expect(
      checkProvider({ name: 'ok', baseUrl: 'https://a.b', models: [big] }).modelRows[big.key]
        ?.context,
    ).toBe('Enter a whole number of tokens');
  });
});

describe('ProvidersPage', () => {
  it('reaches the page from the Integrations link and lists providers', async () => {
    integrationsApi({
      'GET /api/admin/providers': { body: [PROVIDER] },
      'GET /api/admin/model-defaults': { body: NO_DEFAULTS },
    });
    const user = userEvent.setup();
    renderApp('/');
    await user.click(await screen.findByRole('link', { name: 'Integrations' }));
    const nav = await screen.findByRole('navigation', { name: 'Integrations sections' });
    await waitFor(() => {
      expect(within(nav).getByRole('link', { name: 'Providers' })).toHaveAttribute(
        'aria-current',
        'page',
      );
    });
    const row = (await screen.findAllByTestId('providers-table-row'))[0];
    expect(row).toHaveTextContent('Colibri');
    expect(row).toHaveTextContent('http://localhost:11434/v1');
    expect(row).toHaveTextContent('Saved');
    expect(row).toHaveTextContent('2 models');
    expect(row).toHaveTextContent('Not tested');
  });

  it('shows "not found" to members, and hides the link', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_MEMBER } });
    renderApp('/admin/integrations/providers');
    expect(await screen.findByRole('heading', { name: 'Page not found' })).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: 'Integrations' })).toBeNull();
  });

  it('has no accessibility violations with providers, defaults and warnings', async () => {
    const { container } = await openPage({}, [PROVIDER], {
      orchestrator: { provider_name: 'Colibri', model: WEAK_MODEL },
      agents: null,
    });
    expect(await axe(container)).toHaveNoViolations();
  });

  it('says what to do when there are no providers', async () => {
    await openPage({}, []);
    expect(await screen.findByTestId('providers-table-empty')).toHaveTextContent(
      'No providers yet',
    );
    expect(screen.getByTestId('defaults-no-models')).toBeInTheDocument();
  });

  it('shows a load error with retry', async () => {
    const api = integrationsApi({
      'GET /api/admin/providers': { status: 500, body: 'x' },
      'GET /api/admin/model-defaults': { body: NO_DEFAULTS },
    });
    const user = userEvent.setup();
    renderApp('/admin/integrations/providers');
    const error = await screen.findByTestId('providers-load-error');
    api.on('GET /api/admin/providers', { body: [PROVIDER] });
    await user.click(within(error).getByRole('button', { name: 'Try again' }));
    expect(await screen.findByTestId('providers-table')).toBeInTheDocument();
  });

  it('adds a provider with models, sending the key once and never keeping it', async () => {
    const { api } = await openPage(
      {
        'POST /api/admin/providers': { status: 201, body: PROVIDER },
      },
      [],
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId('provider-add'));
    const dialog = screen.getByTestId('provider-dialog');
    expect(dialog).toHaveAttribute('open');
    await user.type(within(dialog).getByTestId('provider-name-input'), 'Colibri');
    await user.type(within(dialog).getByTestId('provider-url-input'), 'http://localhost:11434/v1');
    await user.type(within(dialog).getByTestId('provider-key-input'), SECRET);
    await user.click(within(dialog).getByTestId('model-add'));
    await user.type(within(dialog).getByTestId('model-name-0-input'), 'qwen3-coder-30b');
    await user.selectOptions(within(dialog).getByTestId('model-tool-0-input'), 'supported');
    await user.type(within(dialog).getByTestId('model-context-0-input'), '32768');
    await user.click(within(dialog).getByTestId('provider-save'));
    expect(
      await screen.findByText('Provider added. Run a test to check its models.'),
    ).toBeInTheDocument();
    const post = lastCall(api.calls, 'POST', '/api/admin/providers');
    expect(post?.body).toEqual({
      name: 'Colibri',
      base_url: 'http://localhost:11434/v1',
      api_key: SECRET,
      remove_api_key: false,
      models: [
        {
          id: null,
          name: 'qwen3-coder-30b',
          label: '',
          declared_tool_calling: 'supported',
          declared_context_tokens: 32768,
        },
      ],
    });
    expect(post?.headers.get('x-csrf-token')).toBe('csrf-admin');
    expect(dialog).not.toHaveAttribute('open');
    expect(document.body.innerHTML).not.toContain(SECRET);
    expect(window.localStorage.length).toBe(0);
  });

  it('refuses a bad form without calling the server, with errors beside each field', async () => {
    const { api } = await openPage();
    const user = userEvent.setup();
    await user.click(screen.getByTestId('provider-add'));
    const dialog = screen.getByTestId('provider-dialog');
    await user.type(within(dialog).getByTestId('provider-url-input'), 'http://169.254.169.254');
    await user.click(within(dialog).getByTestId('model-add'));
    await user.click(within(dialog).getByTestId('provider-save'));
    expect(within(dialog).getByTestId('provider-name-error')).toHaveTextContent(
      'up to 60 characters',
    );
    expect(within(dialog).getByTestId('provider-url-error')).toHaveTextContent('always blocked');
    expect(within(dialog).getByTestId('model-name-0-error')).toHaveTextContent(
      'exactly as the provider expects',
    );
    expect(within(dialog).getByTestId('provider-url-input')).toHaveAttribute(
      'aria-invalid',
      'true',
    );
    expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
  });

  it('shows a duplicate name from the server against the name field', async () => {
    await openPage({
      'POST /api/admin/providers': validation('name', 'A provider with this name already exists'),
    });
    const user = userEvent.setup();
    await user.click(screen.getByTestId('provider-add'));
    const dialog = screen.getByTestId('provider-dialog');
    await user.type(within(dialog).getByTestId('provider-name-input'), 'colibri');
    await user.type(within(dialog).getByTestId('provider-url-input'), 'https://api.example.com/v1');
    await user.click(within(dialog).getByTestId('provider-save'));
    expect(await within(dialog).findByTestId('provider-name-error')).toHaveTextContent(
      'already exists',
    );
    expect(within(dialog).getByTestId('provider-dialog-error')).toBeInTheDocument();
  });

  it('edits a provider: an empty key keeps it, and models keep their ids', async () => {
    const { api } = await openPage({
      [`PUT /api/admin/providers/${PROVIDER_ID}`]: { body: PROVIDER },
    });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Edit Colibri' }));
    const dialog = screen.getByTestId('provider-dialog');
    expect(within(dialog).getByRole('heading', { name: 'Edit Colibri' })).toBeInTheDocument();
    expect(within(dialog).getByTestId('provider-key-input')).toHaveValue('');
    expect(within(dialog).getByTestId('provider-key-status')).toHaveTextContent('Saved on');
    expect(within(dialog).getAllByTestId('model-row')).toHaveLength(2);
    expect(within(dialog).getByText(/has not shown tool calling/)).toBeInTheDocument();
    await user.clear(within(dialog).getByTestId('provider-name-input'));
    await user.type(within(dialog).getByTestId('provider-name-input'), 'Colibri box');
    await user.click(within(dialog).getByTestId('model-remove-1'));
    await user.click(within(dialog).getByTestId('provider-save'));
    expect(await screen.findByText('Provider saved')).toBeInTheDocument();
    const put = lastCall(api.calls, 'PUT', `/api/admin/providers/${PROVIDER_ID}`);
    expect(put?.body).toMatchObject({ name: 'Colibri box', remove_api_key: false });
    expect(put?.body).not.toHaveProperty('api_key');
    expect((put?.body as { models: { id: string }[] }).models.map((model) => model.id)).toEqual([
      MODEL_GOOD,
    ]);
  });

  it('removes the saved key only when asked', async () => {
    const { api } = await openPage({
      [`PUT /api/admin/providers/${PROVIDER_ID}`]: { body: { ...PROVIDER, api_key_saved: false } },
    });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Edit Colibri' }));
    const dialog = screen.getByTestId('provider-dialog');
    await user.click(within(dialog).getByTestId('provider-key-remove'));
    await user.click(within(dialog).getByTestId('provider-save'));
    await screen.findByText('Provider saved');
    expect(lastCall(api.calls, 'PUT', `/api/admin/providers/${PROVIDER_ID}`)?.body).toMatchObject({
      remove_api_key: true,
    });
  });

  it('tests a provider in the background and shows per-model results as they arrive', async () => {
    let polls = 0;
    const tested = { ...PROVIDER, last_test: PROVIDER_RUN };
    const { api } = await openPage({
      [`POST /api/admin/providers/${PROVIDER_ID}/test`]: {
        status: 202,
        body: queuedRun(`provider:${PROVIDER_ID}`),
      },
      [`GET /api/admin/test-runs/${RUN_ID}`]: () => {
        polls += 1;
        return {
          body:
            polls === 1 ? { ...PROVIDER_RUN, status: 'running', finished_at: null } : PROVIDER_RUN,
        };
      },
    });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Test Colibri' }));
    const panel = await screen.findByTestId('provider-test-panel');
    api.on('GET /api/admin/providers', { body: [tested] });
    await within(panel).findByText('Model Qwen coder', {}, { timeout: 3000 });
    await waitFor(
      () => {
        expect(within(panel).getByTestId('provider-test-status')).toHaveAttribute(
          'data-status',
          'warning',
        );
      },
      { timeout: 3000 },
    );
    expect(within(panel).getByText('Model tiny-1b')).toBeInTheDocument();
    expect(within(panel).getByText('Tool calling: Unknown')).toBeInTheDocument();
    expect(within(panel).getByTestId('provider-test-warnings')).toHaveTextContent(
      'jobs need at least 8,000',
    );
    await waitFor(() => {
      expect(screen.getAllByTestId('providers-table-row')[0]).not.toHaveTextContent('Testing');
    });
  });

  it('removes a provider after naming the defaults it clears', async () => {
    const { api } = await openPage({
      [`GET /api/admin/providers/${PROVIDER_ID}/removal-impact`]: {
        body: {
          provider_name: 'Colibri',
          model_count: 2,
          cleared_defaults: ['orchestrator', 'agents'],
        },
      },
      [`DELETE /api/admin/providers/${PROVIDER_ID}`]: {
        body: {
          provider_name: 'Colibri',
          model_count: 2,
          cleared_defaults: ['orchestrator', 'agents'],
        },
      },
    });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Remove Colibri' }));
    const dialog = screen.getByTestId('remove-provider');
    expect(await within(dialog).findByTestId('remove-provider-impact')).toHaveTextContent(
      'These defaults will be cleared: Orchestrator, Thread and task agents.',
    );
    expect(dialog).toHaveTextContent('Its 2 models and saved key are deleted');
    api.on('GET /api/admin/providers', { body: [] });
    await user.click(within(dialog).getByTestId('remove-provider-confirm'));
    expect(await screen.findByText('Provider removed')).toBeInTheDocument();
    expect(api.calls.some((call) => call.method === 'DELETE')).toBe(true);
    expect(await screen.findByTestId('providers-table-empty')).toBeInTheDocument();
  });
});

describe('DefaultModelsCard', () => {
  it('names the provider of each default and saves by model id', async () => {
    const { api } = await openPage({
      'PUT /api/admin/model-defaults': {
        body: { orchestrator: { provider_name: 'Colibri', model: GOOD_MODEL }, agents: null },
      },
    });
    const user = userEvent.setup();
    expect(screen.getByTestId('defaults-save')).toBeDisabled();
    await user.selectOptions(screen.getByTestId('defaults-orchestrator-input'), MODEL_GOOD);
    expect(screen.getByTestId('defaults-orchestrator-input')).toHaveAccessibleDescription(
      'From Colibri',
    );
    expect(
      within(screen.getByTestId('defaults-orchestrator-input')).getByRole('option', {
        name: 'Colibri / Qwen coder',
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByTestId('defaults-save'));
    expect(
      await screen.findByText('Default models saved. They apply to new sessions.'),
    ).toBeInTheDocument();
    expect(lastCall(api.calls, 'PUT', '/api/admin/model-defaults')?.body).toEqual({
      orchestrator_model_id: MODEL_GOOD,
      agents_model_id: null,
    });
  });

  it('warns beside an unsuitable pick and needs "Use anyway"', async () => {
    const { api } = await openPage({
      'PUT /api/admin/model-defaults': {
        body: { orchestrator: null, agents: { provider_name: 'Colibri', model: WEAK_MODEL } },
      },
    });
    const user = userEvent.setup();
    await user.selectOptions(screen.getByTestId('defaults-agents-input'), MODEL_WEAK);
    expect(screen.getByTestId('defaults-agents-warnings')).toHaveTextContent(
      'has not shown tool calling',
    );
    await user.click(screen.getByTestId('defaults-save'));
    expect(screen.getByTestId('defaults-problem')).toHaveTextContent('Confirm that you want');
    expect(api.calls.some((call) => call.method === 'PUT')).toBe(false);
    await user.click(screen.getByTestId('defaults-use-anyway'));
    await user.click(screen.getByTestId('defaults-save'));
    expect(await screen.findByText(/Default models saved/)).toBeInTheDocument();
    expect(lastCall(api.calls, 'PUT', '/api/admin/model-defaults')?.body).toEqual({
      orchestrator_model_id: null,
      agents_model_id: MODEL_WEAK,
    });
  });

  it('is operable by keyboard: open the dialog, close it with Escape', async () => {
    await openPage();
    const user = userEvent.setup();
    screen.getByTestId('provider-add').focus();
    await user.keyboard('{Enter}');
    const dialog = screen.getByTestId('provider-dialog');
    expect(dialog).toHaveAttribute('open');
    // jsdom does not move focus into a modal dialog; browsers do (checked end to end).
    dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
    await waitFor(() => {
      expect(dialog).not.toHaveAttribute('open');
    });
  });
});
