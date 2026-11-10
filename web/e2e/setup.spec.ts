import { expect, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { randomClientIp, clientHeaders, strongPassword } from './support/env';
import { totpCode } from './support/totp';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

// Runs once, first, on the fresh database: first-run setup to MFA, then keeps the admin session
// for every other project.
test('first-run setup creates an admin who must enrol MFA before using Tandem', async ({
  page,
}) => {
  await page.goto('/setup');
  await expect(page.getByTestId('setup-page')).toBeVisible();
  expect((await new AxeBuilder({ page }).analyze()).violations).toEqual([]);

  const password = strongPassword();
  await page.getByTestId('setup-name-input').fill('Ada Admin');
  await page.getByTestId('setup-email-input').fill('ada@example.test');
  await page.getByTestId('setup-password-input').fill(password);
  await page.getByTestId('setup-confirm-input').fill(password);
  await page.getByTestId('setup-submit').click();

  // An admin without MFA can reach nothing but enrolment.
  await expect(page).toHaveURL(/\/mfa\/enrol$/);
  await expect(page.getByTestId('mfa-required')).toBeVisible();
  await page.goto('/admin/users');
  await expect(page).toHaveURL(/\/mfa\/enrol$/);

  await page.getByTestId('mfa-start').click();
  await expect(page.getByTestId('mfa-qr')).toBeVisible();
  const secret = (await page.getByTestId('mfa-secret').innerText()).replace(/\s+/g, '');
  expect(secret).toMatch(/^[A-Z2-7]{16,}$/);
  await page.getByTestId('mfa-next').click();
  await page.getByTestId('mfa-code-input').fill(totpCode(secret));
  await page.getByTestId('mfa-confirm').click();

  await expect(page.getByTestId('mfa-codes')).toBeVisible();
  const codes = (await page.getByTestId('recovery-value').innerText()).trim().split('\n');
  expect(codes).toHaveLength(10);
  await expect(page.getByTestId('recovery-done')).toBeDisabled();
  await page.getByTestId('recovery-ack').check();
  await page.getByTestId('recovery-done').click();

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Users' })).toBeVisible();

  // Setup cannot run again.
  await page.goto('/setup');
  await expect(page.getByTestId('not-found-page')).toBeVisible();

  await page.context().storageState({ path: 'e2e/.auth/admin.json' });
});
