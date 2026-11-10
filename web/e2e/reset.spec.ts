import { expect, test } from '@playwright/test';
import {
  adminApi,
  clientHeaders,
  createMember,
  freshContext,
  randomClientIp,
  signInViaUi,
  strongPassword,
  waitOutSignInDelay,
} from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('a forced reset signs the person out everywhere and the link works once', async ({
  page,
  request,
  browser,
}, testInfo) => {
  const member = await createMember(await adminApi(request, testInfo), testInfo, ip);
  const memberContext = await freshContext(browser, testInfo, randomClientIp());
  const memberPage = await memberContext.newPage();
  await signInViaUi(memberPage, member.email, member.password);
  await expect(memberPage.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();

  await page.goto('/admin/users');
  const row = page.getByTestId('users-table-row').filter({ hasText: member.email });
  await row.getByTestId('users-reset-link').click();
  await expect(page.getByTestId('reset-dialog')).toContainText('signs');
  await page.getByTestId('reset-dialog-confirm').click();
  const link = (await page.getByTestId('users-secret-value').innerText()).trim();
  expect(link).toContain('/reset-password?token=');
  await page.getByTestId('users-secret-ack').check();
  await page.getByTestId('users-secret-done').click();

  // The member's existing sign-in is over.
  await memberPage.goto('/chats');
  await expect(memberPage).toHaveURL(/\/sign-in/);

  const newPassword = strongPassword();
  await memberPage.goto(link);
  await expect(memberPage.getByTestId('reset-page')).toBeVisible();
  await expect(memberPage).toHaveURL(/\/reset-password$/);
  await memberPage.getByTestId('reset-password-input').fill(newPassword);
  await memberPage.getByTestId('reset-confirm-input').fill(newPassword);
  await memberPage.getByTestId('reset-submit').click();
  await expect(memberPage).toHaveURL(/\/sign-in$/);
  await expect(memberPage.getByTestId('signin-notice')).toBeVisible();

  const reuse = await memberContext.newPage();
  await reuse.goto(link);
  await expect(reuse.getByTestId('reset-invalid')).toBeVisible();

  await signInViaUi(memberPage, member.email, member.password);
  await expect(memberPage.getByTestId('signin-error')).toContainText(
    'That email and password do not match',
  );
  await waitOutSignInDelay(memberPage);
  await signInViaUi(memberPage, member.email, newPassword);
  await expect(memberPage.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await memberContext.close();
});
