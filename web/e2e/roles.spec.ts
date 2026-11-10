import { expect, test } from '@playwright/test';
import {
  adminApi,
  clientHeaders,
  createMember,
  freshContext,
  randomClientIp,
  signInViaUi,
} from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('a role change applies on the person’s very next page', async ({
  page,
  request,
  browser,
}, testInfo) => {
  const member = await createMember(await adminApi(request, testInfo), testInfo, ip);
  const context = await freshContext(browser, testInfo, randomClientIp());
  const memberPage = await context.newPage();
  await signInViaUi(memberPage, member.email, member.password);
  await expect(memberPage.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await expect(memberPage.getByRole('link', { name: 'Users' })).toHaveCount(0);

  // Admin pages do not exist for members.
  await memberPage.goto('/admin/users');
  await expect(memberPage.getByTestId('not-found-page')).toBeVisible();

  // Promote: an admin without MFA can only enrol.
  await page.goto('/admin/users');
  const row = page.getByTestId('users-table-row').filter({ hasText: member.email });
  await row.getByTestId('users-change-role').click();
  await page.getByTestId('role-select-input').selectOption('admin');
  await page.getByTestId('role-dialog-confirm').click();
  await expect(row).toContainText('admin');

  await memberPage.goto('/chats');
  await expect(memberPage).toHaveURL(/\/mfa\/enrol$/);
  await expect(memberPage.getByTestId('mfa-required')).toBeVisible();

  // Demote again: back to an ordinary member, with no admin navigation.
  await row.getByTestId('users-change-role').click();
  await page.getByTestId('role-select-input').selectOption('member');
  await page.getByTestId('role-dialog-confirm').click();
  await expect(row).toContainText('member');
  await memberPage.goto('/chats');
  await expect(memberPage.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await expect(memberPage.getByRole('link', { name: 'Users' })).toHaveCount(0);
  await context.close();
});
