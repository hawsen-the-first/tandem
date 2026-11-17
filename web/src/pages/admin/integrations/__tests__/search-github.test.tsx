import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { axe } from 'vitest-axe';
import { renderApp } from '../../../../test/render';
import { checkSearchKind } from '../SearchPage';
import { RUN_ID, SECRET, finishedRun, integrationsApi, queuedRun, validation } from './fixtures';

const KIND = {
  active: false,
  active_test_id: null,
  api_key_saved: false,
  api_key_saved_at: null,
  base_url: null,
  configured: false,
  last_test: null,
};
const SEARXNG = { ...KIND, kind: 'searxng', needs_url: true, needs_key: false };
const BRAVE = { ...KIND, kind: 'brave', needs_url: false, needs_key: true };
const TAVILY = { ...KIND, kind: 'tavily', needs_url: false, needs_key: true };
const NONE_ACTIVE = { active: null, kinds: [SEARXNG, BRAVE, TAVILY] };

async function openSearch(extra = {}, overview: unknown = NONE_ACTIVE) {
  const api = integrationsApi({ 'GET /api/admin/search': { body: overview }, ...extra });
  const view = renderApp('/admin/integrations/search');
  await screen.findByTestId('search-card-searxng');
  return { api, ...view };
}

describe('checkSearchKind', () => {
  it('needs a key for Brave and Tavily until one is saved, and a valid SearXNG URL when given', () => {
    const values = { baseUrl: '', apiKey: '', removeKey: false };
    expect(checkSearchKind(BRAVE, values).apiKey).toBe('Enter the key');
    expect(checkSearchKind({ ...BRAVE, api_key_saved: true }, values)).toEqual({});
    expect(
      checkSearchKind({ ...BRAVE, api_key_saved: true }, { ...values, removeKey: true }).apiKey,
    ).toBeDefined();
    expect(checkSearchKind(SEARXNG, values)).toEqual({});
    expect(
      checkSearchKind(SEARXNG, { ...values, baseUrl: 'http://search.example.com' }).baseUrl,
    ).toContain('https://');
  });
});

describe('SearchPage (US-2.5)', () => {
  it('shows three kinds and says search is not set up when none is active', async () => {
    const { container } = await openSearch();
    expect(screen.getByTestId('search-state')).toHaveTextContent(
      'Search is not set up, so @search is unavailable.',
    );
    expect(screen.getByRole('heading', { name: 'SearXNG' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Brave Search' })).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: 'Tavily' })).toBeInTheDocument();
    expect(screen.getByTestId('search-test-brave')).toBeDisabled();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('saves a Brave key without keeping it, then tests and activates it', async () => {
    const savedBrave = {
      ...BRAVE,
      configured: true,
      api_key_saved: true,
      api_key_saved_at: '2026-11-17T00:00:00Z',
    };
    const { api } = await openSearch({
      'PUT /api/admin/search/kinds/brave': {
        body: { active: null, kinds: [SEARXNG, savedBrave, TAVILY] },
      },
      'POST /api/admin/search/kinds/brave/test': { status: 202, body: queuedRun('search:brave') },
      [`GET /api/admin/test-runs/${RUN_ID}`]: {
        body: finishedRun('search:brave', [
          {
            subject: 'search',
            status: 'ok',
            message: 'The search service answered.',
            note: 'no_results',
            details: { repositories: [] },
          },
        ]),
      },
      'PUT /api/admin/search/active': { body: { active: 'brave', untested: false } },
    });
    api.on('GET /api/admin/search', {
      body: { active: null, kinds: [SEARXNG, savedBrave, TAVILY] },
    });
    const user = userEvent.setup();
    const card = screen.getByTestId('search-card-brave');
    await user.click(within(card).getByRole('button', { name: 'Save Brave Search settings' }));
    expect(within(card).getByTestId('search-key-brave-error')).toHaveTextContent('Enter the key');
    await user.type(within(card).getByTestId('search-key-brave-input'), SECRET);
    await user.click(within(card).getByRole('button', { name: 'Save Brave Search settings' }));
    expect(await screen.findByText('Brave Search settings saved')).toBeInTheDocument();
    const put = api.calls.find((call) => call.path === '/api/admin/search/kinds/brave');
    expect(put?.body).toEqual({ api_key: SECRET, remove_api_key: false });
    expect(within(card).getByTestId('search-key-brave-input')).toHaveValue('');
    expect(document.body.innerHTML).not.toContain(SECRET);

    await user.click(within(card).getByRole('button', { name: 'Test Brave Search' }));
    expect(await within(card).findByText(/found nothing for the test query/)).toBeInTheDocument();

    api.on('GET /api/admin/search', {
      body: { active: 'brave', kinds: [SEARXNG, { ...savedBrave, active: true }, TAVILY] },
    });
    await user.click(within(card).getByRole('button', { name: 'Make active: Brave Search' }));
    expect(
      await screen.findByText('Brave Search is now the active search service'),
    ).toBeInTheDocument();
    expect(await screen.findByTestId('search-active-brave')).toHaveTextContent('Active');
    expect(screen.getByTestId('search-state')).toHaveTextContent('Brave Search is active.');
  });

  it('warns when activating a provider that never passed a test', async () => {
    const configured = { ...SEARXNG, configured: true };
    await openSearch(
      { 'PUT /api/admin/search/active': { body: { active: 'searxng', untested: true } } },
      { active: null, kinds: [configured, BRAVE, TAVILY] },
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Make active: SearXNG' }));
    expect(
      await screen.findByText('SearXNG is now active, but it has not passed a test yet'),
    ).toBeInTheDocument();
  });

  it('sends an empty SearXNG URL as "use the bundled service" and shows URL refusals', async () => {
    const { api } = await openSearch({
      'PUT /api/admin/search/kinds/searxng': validation(
        'base_url',
        'That address cannot be used for a server',
      ),
    });
    const user = userEvent.setup();
    const card = screen.getByTestId('search-card-searxng');
    await user.click(within(card).getByRole('button', { name: 'Save SearXNG settings' }));
    expect(api.calls.find((call) => call.path === '/api/admin/search/kinds/searxng')?.body).toEqual(
      {
        base_url: null,
        remove_api_key: false,
      },
    );
    expect(await within(card).findByTestId('search-url-searxng-error')).toHaveTextContent(
      'cannot be used for a server',
    );
  });

  it('turns search off', async () => {
    const { api } = await openSearch(
      { 'PUT /api/admin/search/active': { body: { active: null, untested: false } } },
      { active: 'searxng', kinds: [{ ...SEARXNG, configured: true, active: true }, BRAVE, TAVILY] },
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId('search-turn-off'));
    expect(await screen.findByText('Search is turned off')).toBeInTheDocument();
    expect(api.calls.find((call) => call.path === '/api/admin/search/active')?.body).toEqual({
      kind: null,
    });
  });
});

const NO_TOKEN = { configured: false, active_test_id: null, last_test: null };
const WITH_TOKEN = {
  configured: true,
  saved_at: '2026-11-17T00:00:00Z',
  active_test_id: null,
  last_test: null,
};

async function openGitHub(extra = {}, connector: unknown = NO_TOKEN) {
  const api = integrationsApi({
    'GET /api/admin/connectors/github': { body: connector },
    ...extra,
  });
  const view = renderApp('/admin/integrations/github');
  await screen.findByTestId('github-form');
  return { api, ...view };
}

describe('GitHubPage (US-2.6)', () => {
  it('always shows the minimum permissions and the status', async () => {
    const { container } = await openGitHub();
    expect(screen.getByTestId('github-permissions')).toHaveTextContent(
      'Contents read and write, Pull requests read and write, Metadata read',
    );
    expect(screen.getByTestId('github-permissions')).toHaveTextContent('not recommended');
    expect(screen.getByTestId('github-status')).toHaveTextContent('No token saved');
    expect(screen.getByTestId('github-test')).toBeDisabled();
    expect(screen.queryByTestId('github-remove')).toBeNull();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('saves a token without keeping it, and needs one to save', async () => {
    const { api } = await openGitHub({ 'PUT /api/admin/connectors/github': { body: WITH_TOKEN } });
    const user = userEvent.setup();
    await user.click(screen.getByTestId('github-save'));
    expect(screen.getByTestId('github-token-error')).toHaveTextContent('Enter the token');
    await user.type(screen.getByTestId('github-token-input'), SECRET);
    await user.click(screen.getByTestId('github-save'));
    expect(await screen.findByText(/Token saved. Test the connection/)).toBeInTheDocument();
    expect(api.calls.find((call) => call.method === 'PUT')?.body).toEqual({ token: SECRET });
    expect(screen.getByTestId('github-token-input')).toHaveValue('');
    expect(screen.getByTestId('github-status')).toHaveTextContent('Token saved on');
    expect(document.body.innerHTML).not.toContain(SECRET);
  });

  it('tests the connection and shows the account and repositories', async () => {
    await openGitHub(
      {
        'POST /api/admin/connectors/github/test': {
          status: 202,
          body: queuedRun('connector:github'),
        },
        [`GET /api/admin/test-runs/${RUN_ID}`]: {
          body: finishedRun('connector:github', [
            {
              subject: 'connector',
              status: 'ok',
              message: 'The token works.',
              details: {
                account_login: 'octo',
                repositories: ['octo/tandem'],
                repository_total: 1,
              },
            },
          ]),
        },
      },
      WITH_TOKEN,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId('github-test'));
    expect(await screen.findByText('Account: octo')).toBeInTheDocument();
    expect(screen.getByTestId('test-result-repositories')).toHaveTextContent('octo/tandem');
  });

  it('removes the token after saying what stops working', async () => {
    const { api } = await openGitHub(
      { 'DELETE /api/admin/connectors/github': { status: 204 } },
      WITH_TOKEN,
    );
    const user = userEvent.setup();
    await user.click(screen.getByTestId('github-remove'));
    const dialog = screen.getByTestId('github-remove-dialog');
    expect(dialog).toHaveTextContent('Repository sessions will not be able to clone or push');
    expect(dialog).toHaveTextContent('Managed-folder sessions are not affected');
    api.on('GET /api/admin/connectors/github', { body: NO_TOKEN });
    await user.click(within(dialog).getByTestId('github-remove-dialog-confirm'));
    expect(await screen.findByText('Token removed')).toBeInTheDocument();
    expect(await screen.findByText(/No token saved/)).toBeInTheDocument();
  });
});
