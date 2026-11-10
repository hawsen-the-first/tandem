import { expect, test } from '@playwright/test';
import { clientHeaders, freshContext, randomClientIp } from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('the sign-in form can be completed with the keyboard alone', async ({ browser }, testInfo) => {
  const context = await freshContext(browser, testInfo, randomClientIp());
  const page = await context.newPage();
  await page.goto('/sign-in');
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
    const inside = await page.evaluate(
      () =>
        document.querySelector('[data-testid="invite-dialog"]')?.contains(document.activeElement) ??
        false,
    );
    expect(inside, 'focus stays inside the open dialog').toBe(true);
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
  await expect(page.getByRole('link', { name: 'Chats' })).toBeHidden();
  await menu.focus();
  await page.keyboard.press('Enter');
  await expect(menu).toHaveAttribute('aria-expanded', 'true');
  await expect(page.getByRole('link', { name: 'Chats' })).toBeVisible();
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
