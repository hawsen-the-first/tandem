import { expect, test } from '@playwright/test';
import {
  clientHeaders,
  freshContext,
  randomClientIp,
  signInViaUi,
  strongPassword,
  uniqueEmail,
} from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('an admin invites someone, who accepts, signs out and signs in again', async ({
  page,
  browser,
}, testInfo) => {
  const email = uniqueEmail('invitee');
  const password = strongPassword();

  await page.goto('/admin/users');
  await page.getByTestId('users-invite').click();
  await page.getByTestId('invite-email-input').fill(email);
  await page.getByTestId('invite-dialog-confirm').click();

  // The link is shown once and must be acknowledged before it can be dismissed.
  const link = (await page.getByTestId('users-secret-value').innerText()).trim();
  expect(link).toContain('/accept-invitation?token=');
  await expect(page.getByTestId('users-secret-done')).toBeDisabled();
  await page.getByTestId('users-secret-ack').check();
  await page.getByTestId('users-secret-done').click();
  await expect(page.getByTestId('users-secret-value')).toHaveCount(0);
  await expect(page.getByTestId('users-table-row').filter({ hasText: email })).toContainText(
    'Pending',
  );

  // The invitee opens the link in a clean browser.
  const context = await freshContext(browser, testInfo, randomClientIp());
  const invitee = await context.newPage();
  await invitee.goto(link);
  await expect(invitee.getByTestId('invite-page')).toBeVisible();
  await expect(invitee).toHaveURL(/\/accept-invitation$/); // the token left the address bar
  await invitee.getByTestId('invite-name-input').fill('Ines Invitee');
  await invitee.getByTestId('invite-password-input').fill(password);
  await invitee.getByTestId('invite-confirm-input').fill(password);
  await invitee.getByTestId('invite-submit').click();
  await expect(invitee.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await expect(invitee.getByRole('link', { name: 'Users' })).toHaveCount(0);

  // The link works once.
  const again = await context.newPage();
  await again.goto(link);
  await expect(again.getByTestId('invite-invalid')).toBeVisible();

  await invitee.getByText('Ines Invitee').click();
  await invitee.getByTestId('sign-out').click();
  await expect(invitee).toHaveURL(/\/sign-in$/);
  await signInViaUi(invitee, email, password);
  await expect(invitee.getByRole('navigation', { name: 'Main navigation' })).toBeVisible();
  await context.close();

  // The admin sees the new person as active.
  await page.goto('/admin/users');
  await expect(page.getByTestId('users-table-row').filter({ hasText: email })).toContainText(
    'Active',
  );
});

test('a link that is not valid says so without revealing why', async ({ browser }, testInfo) => {
  const context = await freshContext(browser, testInfo, randomClientIp());
  const page = await context.newPage();
  await page.goto('/accept-invitation?token=definitely-not-a-real-token');
  await expect(page.getByTestId('invite-invalid')).toContainText('This link is not valid');
  await context.close();
});
