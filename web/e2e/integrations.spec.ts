import AxeBuilder from '@axe-core/playwright';
import { randomBytes } from 'node:crypto';
import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import {
  adminApi,
  clientHeaders,
  createMember,
  freshContext,
  randomClientIp,
  signedInViaUi,
} from './support/env';

// U2 scenarios against the real stack. dev/scripts/e2e.sh starts stand-in services on loopback
// (e2e/mocks/server.mjs): an OpenAI-compatible provider with a capable and a weak model, and a
// SearXNG instance. The suite runs once per browser against one database, so names are unique
// and every scenario leaves the instance as it found it.

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

const MOCKS = process.env['E2E_MOCK_URL'] ?? 'http://127.0.0.1:8092';
const TEST_LIMIT = 30_000;

function unique(prefix: string): string {
  return `${prefix} ${randomBytes(3).toString('hex')}`;
}

function row(page: Page, name: string) {
  return page.getByTestId('providers-table-row').filter({ hasText: name });
}

async function addProvider(page: Page, name: string): Promise<void> {
  await page.goto('/admin/integrations/providers');
  await page.getByTestId('provider-add').click();
  const dialog = page.getByTestId('provider-dialog');
  await dialog.getByTestId('provider-name-input').fill(name);
  await dialog.getByTestId('provider-url-input').fill(`${MOCKS}/v1`);
  await dialog.getByTestId('model-add').click();
  await dialog.getByTestId('model-name-0-input').fill('capable');
  await dialog.getByTestId('model-add').click();
  await dialog.getByTestId('model-name-1-input').fill('weak');
  await dialog.getByTestId('provider-save').click();
  await expect(page.getByText('Provider added. Run a test to check its models.')).toBeVisible();
  await expect(row(page, name)).toBeVisible();
}

async function removeProvider(page: Page, name: string): Promise<void> {
  await page.goto('/admin/integrations/providers');
  await page.getByRole('button', { name: `Remove ${name}` }).click();
  const dialog = page.getByTestId('remove-provider');
  await expect(dialog.getByTestId('remove-provider-impact')).toBeVisible();
  await dialog.getByTestId('remove-provider-confirm').click();
  await expect(page.getByText('Provider removed')).toBeVisible();
  await expect(row(page, name)).toHaveCount(0);
}

test('add a provider, test it, read its capabilities, set defaults, remove it', async ({
  page,
}) => {
  const name = unique('Mock provider');
  await addProvider(page, name);

  await page.getByRole('button', { name: `Test ${name}` }).click();
  const panel = page.getByTestId('provider-test-panel');
  await expect(panel.getByTestId('provider-test-status')).toHaveAttribute(
    'data-status',
    'warning',
    {
      timeout: TEST_LIMIT,
    },
  );
  const capable = panel.getByTestId('provider-test-item').filter({ hasText: 'Model capable' });
  await expect(capable).toContainText('Tool calling: Supported');
  await expect(capable).toContainText('Context: 32,768 tokens');
  const weak = panel.getByTestId('provider-test-item').filter({ hasText: 'Model weak' });
  await expect(weak).toContainText('Tool calling: Not supported');
  await expect(panel.getByTestId('provider-test-warnings')).toContainText(
    'does not support tool calling',
  );

  // A suitable orchestrator, and agents on the weak model with the warning acknowledged.
  const orchestrator = page.getByTestId('defaults-orchestrator-input');
  await orchestrator.selectOption({ label: `${name} / capable` });
  await expect(page.getByTestId('defaults-orchestrator-warnings')).toHaveCount(0);
  await page.getByTestId('defaults-agents-input').selectOption({ label: `${name} / weak` });
  await expect(page.getByTestId('defaults-agents-warnings')).toContainText(
    'does not support tool calling',
  );
  await page.getByTestId('defaults-save').click();
  await expect(page.getByTestId('defaults-problem')).toBeVisible();
  await page.getByTestId('defaults-use-anyway').check();
  await page.getByTestId('defaults-save').click();
  await expect(page.getByText('Default models saved. They apply to new sessions.')).toBeVisible();
  await page.reload();
  await expect(page.getByTestId('defaults-orchestrator-input')).toHaveAccessibleDescription(
    `From ${name}`,
  );

  // Removing the provider names both defaults and clears them.
  await page.getByRole('button', { name: `Remove ${name}` }).click();
  const dialog = page.getByTestId('remove-provider');
  await expect(dialog.getByTestId('remove-provider-impact')).toContainText(
    'Orchestrator, Thread and task agents',
  );
  await dialog.getByTestId('remove-provider-confirm').click();
  await expect(page.getByText('Provider removed')).toBeVisible();
  await expect(row(page, name)).toHaveCount(0);
  const defaults = page.getByTestId('defaults-card');
  if ((await page.getByTestId('defaults-orchestrator-input').count()) > 0) {
    await expect(page.getByTestId('defaults-orchestrator-input')).toHaveValue('');
    await expect(page.getByTestId('defaults-agents-input')).toHaveValue('');
  } else {
    await expect(defaults.getByTestId('defaults-no-models')).toBeVisible();
  }
});

test('a provider key is never shown again and a bad URL is refused with its reason', async ({
  page,
}) => {
  const name = unique('Keyed provider');
  const key = `sk-e2e-${randomBytes(12).toString('hex')}`;
  await page.goto('/admin/integrations/providers');
  await page.getByTestId('provider-add').click();
  const dialog = page.getByTestId('provider-dialog');
  await dialog.getByTestId('provider-name-input').fill(name);
  await dialog.getByTestId('provider-url-input').fill('http://api.example.com/v1');
  await dialog.getByTestId('provider-save').click();
  await expect(dialog.getByTestId('provider-url-error')).toContainText(
    'Use https:// for public servers',
  );
  await dialog.getByTestId('provider-url-input').fill(`${MOCKS}/v1`);
  await dialog.getByTestId('provider-key-input').fill(key);
  await dialog.getByTestId('provider-save').click();
  await expect(row(page, name)).toContainText('Saved');

  await page.getByRole('button', { name: `Edit ${name}` }).click();
  await expect(dialog.getByTestId('provider-key-input')).toHaveValue('');
  await expect(dialog.getByTestId('provider-key-status')).toContainText('Saved on');
  expect(await page.content()).not.toContain(key);
  await dialog.getByTestId('provider-cancel').click();
  await removeProvider(page, name);
});

test('switch the search provider and test it', async ({ page }) => {
  await page.goto('/admin/integrations/search');
  const card = page.getByTestId('search-card-searxng');
  await card.getByTestId('search-url-searxng-input').fill(MOCKS);
  await card.getByRole('button', { name: 'Save SearXNG settings' }).click();
  await expect(page.getByText('SearXNG settings saved')).toBeVisible();
  await card.getByRole('button', { name: 'Test SearXNG' }).click();
  await expect(card.getByTestId('search-result-searxng-status')).toHaveAttribute(
    'data-status',
    'ok',
    {
      timeout: TEST_LIMIT,
    },
  );
  const activate = card.getByRole('button', { name: 'Make active: SearXNG' });
  if ((await activate.count()) > 0) {
    await activate.click();
    await expect(page.getByText('SearXNG is now the active search service')).toBeVisible();
  }
  await expect(card.getByTestId('search-active-searxng')).toBeVisible();
  await expect(page.getByTestId('search-state')).toContainText('SearXNG is active.');

  await page.getByTestId('search-turn-off').click();
  await expect(page.getByText('Search is turned off')).toBeVisible();
  await expect(page.getByTestId('search-state')).toContainText('@search is unavailable');
});

test('save and remove the GitHub token', async ({ page }) => {
  const token = `github_pat_e2e_${randomBytes(12).toString('hex')}`;
  await page.goto('/admin/integrations/github');
  await expect(page.getByTestId('github-permissions')).toContainText('Metadata read');
  await page.getByTestId('github-token-input').fill(token);
  await page.getByTestId('github-save').click();
  await expect(page.getByText(/Token saved. Test the connection/)).toBeVisible();
  await expect(page.getByTestId('github-token-input')).toHaveValue('');
  await page.reload();
  await expect(page.getByTestId('github-status')).toContainText('Token saved on');
  expect(await page.content()).not.toContain(token);

  await page.getByTestId('github-remove').click();
  const dialog = page.getByTestId('github-remove-dialog');
  await expect(dialog).toContainText('Managed-folder sessions are not affected');
  await dialog.getByTestId('github-remove-dialog-confirm').click();
  await expect(page.getByText('Token removed')).toBeVisible();
  await expect(page.getByTestId('github-status')).toContainText('No token saved');
});

test('edit the sandbox policy, reset a limit, and find the container tier unavailable', async ({
  page,
}) => {
  await page.goto('/admin/integrations/sandbox-policy');
  await expect(page.getByTestId('policy-applies')).toBeVisible();
  await expect(page.getByTestId('policy-tool-command_run')).toBeDisabled();
  const limit = page.getByTestId('policy-limit-wall_clock_seconds-input');
  const original = await limit.inputValue();
  const changed = original === '90' ? '91' : '90';
  await limit.fill(changed);
  await page.getByTestId('policy-save').click();
  await expect(page.getByText('Sandbox policy saved and recorded in the audit log.')).toBeVisible();
  await page.reload();
  await expect(limit).toHaveValue(changed);

  await page.getByRole('button', { name: 'Reset to default: Time per run' }).click();
  await expect(limit).not.toHaveValue(changed);
  await page.getByTestId('policy-save').click();
  await expect(page.getByText('Sandbox policy saved and recorded in the audit log.')).toBeVisible();

  // The e2e worker has no container socket configured, so the runtime check fails closed.
  await expect(page.getByTestId('policy-tier-container')).toBeDisabled();
  await page.getByTestId('policy-check-runtime').click();
  await expect(page.getByTestId('policy-runtime-unavailable')).toBeVisible({ timeout: TEST_LIMIT });
  await expect(page.getByTestId('policy-tier-container')).toBeDisabled();
});

test('the integrations pages are "not found" for members, and the API agrees', async ({
  browser,
  request,
}, testInfo) => {
  const admin = await adminApi(request, testInfo);
  const memberIp = randomClientIp();
  const member = await createMember(admin, testInfo, memberIp, 'Ivy Integrations');
  const context = await freshContext(browser, testInfo, memberIp);
  const page = await context.newPage();
  await signedInViaUi(page, member.email, member.password);
  await expect(page.getByRole('link', { name: 'Integrations' })).toHaveCount(0);
  for (const path of [
    '/admin/integrations/providers',
    '/admin/integrations/search',
    '/admin/integrations/github',
    '/admin/integrations/sandbox-policy',
  ]) {
    await page.goto(path);
    await expect(page.getByRole('heading', { name: 'Page not found' })).toBeVisible();
  }
  const api = await page.request.get('/api/admin/providers');
  expect(api.status()).toBe(404);
  await context.close();
});

test('the dialogs have no accessibility violations in either theme', async ({ page }) => {
  const name = unique('Axe provider');
  await addProvider(page, name);
  for (const theme of ['light', 'dark'] as const) {
    await page.evaluate((value) => {
      window.localStorage.setItem('tandem.theme', value);
    }, theme);
    await page.reload();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    await page.getByRole('button', { name: `Edit ${name}` }).click();
    const dialog = page.getByTestId('provider-dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog.getByTestId('provider-name-input')).toBeFocused();
    const results = await new AxeBuilder({ page })
      .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
      .analyze();
    expect(
      results.violations.map((violation) => violation.id),
      `${theme} dialog`,
    ).toEqual([]);
    await page.keyboard.press('Escape');
    await expect(dialog).toBeHidden();
    await expect(page.getByRole('button', { name: `Edit ${name}` })).toBeFocused();
  }
  await removeProvider(page, name);
});
