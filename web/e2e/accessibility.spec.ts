import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { clientHeaders, freshContext, randomClientIp } from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

const TAGS = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'];

async function scan(page: Page, label: string): Promise<void> {
  const results = await new AxeBuilder({ page }).withTags(TAGS).analyze();
  const summary = results.violations.map(
    (violation) =>
      `${violation.id}: ${violation.nodes.map((node) => node.target.join(' ')).join(' | ')}`,
  );
  expect(summary, `${label}: accessibility violations`).toEqual([]);
}

async function overflow(page: Page): Promise<number> {
  return page.evaluate(
    () => document.documentElement.scrollWidth - document.documentElement.clientWidth,
  );
}

const SIGNED_IN_PAGES = [
  '/',
  '/chats',
  '/account',
  '/admin/users',
  '/admin/settings',
  '/admin/audit',
];

for (const theme of ['light', 'dark'] as const) {
  test.describe(`${theme} theme`, () => {
    test.beforeEach(async ({ page }) => {
      await page.addInitScript((value) => {
        window.localStorage.setItem('tandem.theme', value);
      }, theme);
    });

    for (const path of SIGNED_IN_PAGES) {
      test(`signed-in page ${path} has no accessibility violations`, async ({ page }) => {
        await page.goto(path);
        await expect(page.getByRole('main')).toBeVisible();
        await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
        // Wait for the page's own data so the scan sees the real content.
        await page.waitForLoadState('networkidle');
        await scan(page, path);
      });
    }

    test('public pages have no accessibility violations', async ({ browser }, testInfo) => {
      const context = await freshContext(browser, testInfo, randomClientIp());
      await context.addInitScript((value) => {
        window.localStorage.setItem('tandem.theme', value);
      }, theme);
      const page = await context.newPage();
      for (const path of [
        '/sign-in',
        '/accept-invitation?token=nope',
        '/reset-password?token=nope',
      ]) {
        await page.goto(path);
        await page.waitForLoadState('networkidle');
        await scan(page, path);
      }
      // Validation errors are part of the page too.
      await page.goto('/sign-in');
      await page.getByTestId('signin-submit').click();
      await expect(page.getByTestId('signin-email-error')).toBeVisible();
      await scan(page, '/sign-in with errors');
      await context.close();
    });
  });
}

test('every signed-in page fits a 360 px screen without sideways scrolling', async ({
  browser,
}, testInfo) => {
  const context = await browser.newContext({
    baseURL: String(testInfo.project.use.baseURL),
    extraHTTPHeaders: clientHeaders(ip),
    storageState: 'e2e/.auth/admin.json',
    viewport: { width: 360, height: 740 },
  });
  const page = await context.newPage();
  for (const path of SIGNED_IN_PAGES) {
    await page.goto(path);
    await expect(page.getByRole('main')).toBeVisible();
    await page.waitForLoadState('networkidle');
    expect(await overflow(page), `${path} overflows`).toBeLessThanOrEqual(0);
  }
  await context.close();
});
