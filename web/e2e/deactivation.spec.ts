import { expect, test } from '@playwright/test';
import {
  adminApi,
  clientHeaders,
  createMember,
  freshContext,
  randomClientIp,
  signInViaUi,
  waitOutSignInDelay,
} from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('deactivating signs the person out at once and reactivating lets them back in', async ({
  page,
  request,
  browser,
}, testInfo) => {
  const member = await createMember(await adminApi(request, testInfo), testInfo, ip);
  const context = await freshContext(browser, testInfo, randomClientIp());
  const memberPage = await context.newPage();
  await signInViaUi(memberPage, member.email, member.password);
  await expect(memberPage.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();

  await page.goto('/admin/users');
  const row = page.getByTestId('users-table-row').filter({ hasText: member.email });
  await row.getByTestId('users-deactivate').click();
  const confirm = page.getByTestId('action-dialog-confirm');
  await expect(confirm).toBeDisabled();
  await page.getByTestId('action-dialog-typed').fill(member.email);
  await confirm.click();
  await expect(row).toContainText('Deactivated');

  await memberPage.goto('/chats');
  await expect(memberPage).toHaveURL(/\/sign-in/);
  await signInViaUi(memberPage, member.email, member.password);
  await expect(memberPage.getByTestId('signin-error')).toContainText(
    'That email and password do not match',
  );

  await row.getByTestId('users-reactivate').click();
  await page.getByTestId('action-dialog-confirm').click();
  await expect(row).toContainText('Active');
  await waitOutSignInDelay(memberPage);
  await signInViaUi(memberPage, member.email, member.password);
  await expect(memberPage.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await context.close();
});

test('the only admin cannot be demoted or deactivated', async ({ page }) => {
  await page.goto('/admin/users');
  const row = page.getByTestId('users-table-row').filter({ hasText: 'ada@example.test' });
  await expect(row.getByTestId('users-deactivate')).toBeDisabled();
  await expect(row.getByTestId('users-change-role')).toBeDisabled();
  await expect(row.getByTestId('users-reason')).toBeVisible();
});
