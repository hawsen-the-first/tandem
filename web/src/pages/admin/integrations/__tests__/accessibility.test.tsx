import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { renderApp } from '../../../../test/render';
import { NO_DEFAULTS, PROVIDER, PROVIDER_ID, PROVIDER_RUN, integrationsApi } from './fixtures';

// Step 38: every dialog has an axe check, each page can be walked by keyboard, and headings and
// live regions are in place. Page-level axe checks live with each page's tests; both themes and
// 360 px widths are checked end to end (e2e/accessibility.spec.ts).

async function openProviders() {
  integrationsApi({
    'GET /api/admin/providers': { body: [{ ...PROVIDER, last_test: PROVIDER_RUN }] },
    'GET /api/admin/model-defaults': { body: NO_DEFAULTS },
    [`GET /api/admin/providers/${PROVIDER_ID}/removal-impact`]: {
      body: { provider_name: 'Colibri', model_count: 2, cleared_defaults: [] },
    },
  });
  const view = renderApp('/admin/integrations/providers');
  await screen.findByTestId('providers-table');
  return view;
}

describe('integrations accessibility', () => {
  it('has one h1 per page and labelled sections', async () => {
    await openProviders();
    expect(screen.getAllByRole('heading', { level: 1 })).toHaveLength(1);
    expect(screen.getByRole('heading', { level: 2, name: 'Default models' })).toBeInTheDocument();
    expect(screen.getByRole('navigation', { name: 'Integrations sections' })).toBeInTheDocument();
  });

  it('the provider dialog has no violations, empty or with errors', async () => {
    const { container } = await openProviders();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Edit Colibri' }));
    const dialog = screen.getByTestId('provider-dialog');
    expect(dialog).toHaveAccessibleName('Edit Colibri');
    expect(within(dialog).getByRole('group', { name: 'Model 1' })).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
    await user.clear(within(dialog).getByTestId('provider-name-input'));
    await user.click(within(dialog).getByTestId('model-add'));
    await user.click(within(dialog).getByTestId('provider-save'));
    expect(within(dialog).getByTestId('provider-name-error')).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('the remove dialog has no violations', async () => {
    const { container } = await openProviders();
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Remove Colibri' }));
    await screen.findByTestId('remove-provider-impact');
    expect(screen.getByTestId('remove-provider')).toHaveAccessibleName('Remove Colibri?');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('the last test result is announced politely and readable by its badge text', async () => {
    await openProviders();
    const user = userEvent.setup();
    expect(screen.getAllByTestId('providers-table-row')[0]).toHaveTextContent('Warning');
    integrationsApi({
      'GET /api/admin/providers': { body: [{ ...PROVIDER, last_test: PROVIDER_RUN }] },
      'GET /api/admin/model-defaults': { body: NO_DEFAULTS },
      [`POST /api/admin/providers/${PROVIDER_ID}/test`]: {
        status: 409,
        body: {
          error: {
            code: 'already_running',
            message: 'A test is already running.',
            run_id: PROVIDER_RUN.id,
          },
        },
      },
      [`GET /api/admin/test-runs/${PROVIDER_RUN.id}`]: { body: PROVIDER_RUN },
    });
    await user.click(screen.getByRole('button', { name: 'Test Colibri' }));
    const panel = await screen.findByTestId('provider-test-panel');
    expect(within(panel).getByRole('status')).toHaveAttribute('aria-live', 'polite');
  });

  it('the GitHub page can be used by keyboard alone, and its remove dialog has no violations', async () => {
    const api = integrationsApi({
      'GET /api/admin/connectors/github': {
        body: {
          configured: true,
          saved_at: '2026-11-17T00:00:00Z',
          active_test_id: null,
          last_test: null,
        },
      },
      'PUT /api/admin/connectors/github': {
        body: {
          configured: true,
          saved_at: '2026-11-17T01:00:00Z',
          active_test_id: null,
          last_test: null,
        },
      },
    });
    const { container } = renderApp('/admin/integrations/github');
    await screen.findByTestId('github-form');
    const user = userEvent.setup();
    screen.getByTestId('github-token-input').focus();
    await user.keyboard('ghp_example');
    await user.tab();
    expect(screen.getByTestId('github-save')).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(await screen.findByText(/Token saved. Test the connection/)).toBeInTheDocument();
    expect(api.calls.some((call) => call.method === 'PUT')).toBe(true);
    await user.click(screen.getByTestId('github-remove'));
    expect(screen.getByTestId('github-remove-dialog')).toHaveAccessibleName(
      'Remove the GitHub token?',
    );
    expect(await axe(container)).toHaveNoViolations();
  });

  it('the search page can be used by keyboard alone', async () => {
    const kind = {
      active: false,
      active_test_id: null,
      api_key_saved: false,
      api_key_saved_at: null,
      base_url: null,
      configured: false,
      last_test: null,
    };
    const api = integrationsApi({
      'GET /api/admin/search': {
        body: {
          active: null,
          kinds: [
            { ...kind, kind: 'searxng', needs_url: true, needs_key: false },
            { ...kind, kind: 'brave', needs_url: false, needs_key: true },
            { ...kind, kind: 'tavily', needs_url: false, needs_key: true },
          ],
        },
      },
      'PUT /api/admin/search/kinds/searxng': (request) => ({
        body: {
          active: null,
          kinds: [
            {
              ...kind,
              kind: 'searxng',
              needs_url: true,
              needs_key: false,
              configured: true,
              base_url: (request.body as { base_url: string }).base_url,
            },
          ],
        },
      }),
    });
    renderApp('/admin/integrations/search');
    await screen.findByTestId('search-card-searxng');
    const user = userEvent.setup();
    screen.getByTestId('search-url-searxng-input').focus();
    await user.keyboard('http://localhost:8888');
    await user.tab();
    expect(screen.getByRole('button', { name: 'Save SearXNG settings' })).toHaveFocus();
    await user.keyboard('{Enter}');
    expect(await screen.findByText('SearXNG settings saved')).toBeInTheDocument();
    expect(api.calls.find((call) => call.method === 'PUT')?.body).toEqual({
      base_url: 'http://localhost:8888',
      remove_api_key: false,
    });
  });
});
