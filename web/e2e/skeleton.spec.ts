import { expect, test } from '@playwright/test';
import {
  adminApi,
  clientHeaders,
  createMember,
  freshContext,
  randomClientIp,
  signedInViaUi,
} from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('a message travels web, API, database, worker and back over the WebSocket', async ({
  request,
  browser,
}, testInfo) => {
  const member = await createMember(await adminApi(request, testInfo), testInfo, ip);
  const context = await freshContext(browser, testInfo, randomClientIp());
  const page = await context.newPage();
  await signedInViaUi(page, member.email, member.password);

  await page
    .getByRole('navigation', { name: 'Main navigation' })
    .getByRole('link', { name: 'Chats', exact: true })
    .click();
  await page.getByTestId('chat-title-input').fill('Round trip');
  await page.getByTestId('chat-create').click();
  // Creating a chat opens it.
  await expect(page).toHaveURL(/\/chats\/[0-9a-f-]+$/);
  await expect(page.getByTestId('chat-session-page')).toBeVisible();

  const composer = page.getByTestId('chat-composer-input');
  await composer.fill('hello team');
  await page.getByTestId('chat-send').click();

  const messages = page.getByTestId('chat-message');
  await expect(messages).toHaveCount(2, { timeout: 15_000 });
  await expect(messages.nth(0)).toContainText('hello team');
  await expect(messages.nth(1)).toContainText('Received: hello team');
  await expect(messages.nth(1).getByTestId('chat-message-author')).toContainText('mock-agent');

  await page.reload();
  await expect(page.getByTestId('chat-message')).toHaveCount(2);

  // Enter sends, Shift+Enter adds a line.
  await page.getByTestId('chat-composer-input').fill('line one');
  await page.getByTestId('chat-composer-input').press('Shift+Enter');
  await page.getByTestId('chat-composer-input').pressSequentially('line two');
  await page.getByTestId('chat-composer-input').press('Enter');
  await expect(page.getByTestId('chat-message')).toHaveCount(4, { timeout: 15_000 });
  await context.close();
});

test('members cannot see each other’s chats', async ({ request, browser }, testInfo) => {
  const admin = await adminApi(request, testInfo);
  const owner = await createMember(admin, testInfo, ip, 'Olive Owner');
  const other = await createMember(admin, testInfo, ip, 'Otto Other');
  const ownerContext = await freshContext(browser, testInfo, randomClientIp());
  const ownerPage = await ownerContext.newPage();
  await signedInViaUi(ownerPage, owner.email, owner.password);
  await ownerPage.goto('/chats');
  await ownerPage.getByTestId('chat-title-input').fill('Private notes');
  await ownerPage.getByTestId('chat-create').click();
  await expect(ownerPage).toHaveURL(/\/chats\/[0-9a-f-]+$/);
  const chatUrl = ownerPage.url();

  const otherContext = await freshContext(browser, testInfo, randomClientIp());
  const otherPage = await otherContext.newPage();
  await signedInViaUi(otherPage, other.email, other.password);
  await otherPage.goto(chatUrl);
  await expect(otherPage.getByTestId('chat-messages-error')).toBeVisible();
  await otherPage.goto('/chats');
  await expect(otherPage.getByTestId('chat-list-page')).toBeVisible();
  await expect(otherPage.getByTestId('chat-list-loading')).toHaveCount(0);
  await expect(otherPage.getByText('Private notes')).toHaveCount(0);
  await expect(otherPage.getByTestId('chat-list')).toHaveCount(0); // no chats of their own
  await ownerContext.close();
  await otherContext.close();
});
