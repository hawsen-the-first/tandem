import { expect, test } from '@playwright/test';
import type { Page } from '@playwright/test';
import { clientHeaders, freshContext, randomClientIp } from './support/env';

const ip = randomClientIp();

// Start from a known position: the document body, so the next Tab goes to the first focusable
// element (headless browsers do not always start with the page focused).
async function focusPage(page: Page): Promise<void> {
  await page.evaluate(() => {
    const body = document.body;
    body.setAttribute('tabindex', '-1');
    body.focus();
    body.removeAttribute('tabindex');
  });
}
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('the sign-in form can be completed with the keyboard alone', async ({ browser }, testInfo) => {
  const context = await freshContext(browser, testInfo, randomClientIp());
  const page = await context.newPage();
  await page.goto('/sign-in');
  // Clicking the heading sets where keyboard navigation starts, in every browser.
  await page.getByRole('heading', { level: 1 }).click();
  await page.keyboard.press('Tab');
  await expect(page.getByTestId('signin-email-input')).toBeFocused();
  await page.keyboard.type('someone@example.test');
  await page.keyboard.press('Tab');
  await expect(page.getByTestId('signin-password-input')).toBeFocused();
  await page.keyboard.type('Not-The-Password-1!');
  await page.keyboard.press('Tab'); // the show/hide control
  await page.keyboard.press('Tab');
  await expect(page.getByTestId('signin-submit')).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('signin-error')).toContainText(
    'That email and password do not match',
  );
  await context.close();
});

test('an empty form moves focus to the first problem', async ({ browser }, testInfo) => {
  const context = await freshContext(browser, testInfo, randomClientIp());
  const page = await context.newPage();
  await page.goto('/sign-in');
  await page.getByTestId('signin-submit').click();
  await expect(page.getByTestId('signin-email-input')).toBeFocused();
  await expect(page.getByTestId('signin-email-input')).toHaveAttribute('aria-invalid', 'true');
  await context.close();
});

test('the skip link jumps over the navigation to the content', async ({ page }) => {
  await page.goto('/account');
  await focusPage(page);
  await page.keyboard.press('Tab');
  const skip = page.getByRole('link', { name: 'Skip to content' });
  await expect(skip).toBeFocused();
  await expect(skip).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(/#main$/);
});

test('a dialog opens from the keyboard, traps focus, closes on Escape and returns focus', async ({
  page,
}) => {
  await page.goto('/admin/users');
  const invite = page.getByTestId('users-invite');
  await invite.focus();
  await page.keyboard.press('Enter');
  const dialog = page.getByTestId('invite-dialog');
  await expect(dialog).toBeVisible();
  for (let index = 0; index < 8; index += 1) {
    await page.keyboard.press('Tab');
    // A native modal dialog makes the page behind it inert: focus may step out to the browser
    // itself (the active element is then the body) but never onto the page behind the dialog.
    const onBackground = await page.evaluate(() => {
      const active = document.activeElement;
      const dialogElement = document.querySelector('[data-testid="invite-dialog"]');
      return (
        active !== null && active !== document.body && dialogElement?.contains(active) !== true
      );
    });
    expect(onBackground, 'focus never reaches the page behind the dialog').toBe(false);
  }
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(invite).toBeFocused();
});

test('the narrow-screen menu works from the keyboard', async ({ browser }, testInfo) => {
  const context = await browser.newContext({
    baseURL: String(testInfo.project.use.baseURL),
    extraHTTPHeaders: clientHeaders(ip),
    storageState: 'e2e/.auth/admin.json',
    viewport: { width: 360, height: 740 },
  });
  const page = await context.newPage();
  await page.goto('/');
  const menu = page.getByTestId('menu-button');
  await expect(menu).toHaveAttribute('aria-expanded', 'false');
  await expect(
    page
      .getByRole('navigation', { name: 'Main navigation' })
      .getByRole('link', { name: 'Chats', exact: true }),
  ).toBeHidden();
  await menu.focus();
  await page.keyboard.press('Enter');
  await expect(menu).toHaveAttribute('aria-expanded', 'true');
  await expect(
    page
      .getByRole('navigation', { name: 'Main navigation' })
      .getByRole('link', { name: 'Chats', exact: true }),
  ).toBeVisible();
  await context.close();
});

test('the theme choice sticks across a reload', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('theme-toggle').selectOption('dark');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page.getByTestId('theme-toggle').selectOption('system');
  await expect(page.locator('html')).not.toHaveAttribute('data-theme', /.+/);
});

test('every page ends with the legal notices, the source link and the credit', async ({
  page,
  browser,
}, testInfo) => {
  for (const path of ['/', '/account', '/admin/users']) {
    await page.goto(path);
    const footer = page.getByTestId('site-footer');
    await expect(footer).toContainText('ABSOLUTELY NO WARRANTY');
    await expect(footer.getByTestId('footer-license')).toHaveAttribute('href', /agpl-3\.0/);
    await expect(footer.getByTestId('footer-source')).toBeVisible();
    await expect(footer).toContainText('Based on Tandem');
  }
  const context = await freshContext(browser, testInfo, randomClientIp());
  const anonymous = await context.newPage();
  await anonymous.goto('/sign-in');
  await expect(anonymous.getByTestId('site-footer')).toContainText('Based on Tandem');
  await context.close();
});
