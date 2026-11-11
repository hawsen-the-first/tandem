import { expect, test } from '@playwright/test';
import { adminApi, clientHeaders, createMember, freshContext, randomClientIp } from './support/env';

const ip = randomClientIp();
test.use({ extraHTTPHeaders: clientHeaders(ip) });

test('repeated wrong passwords show a countdown, disable the button and use one wording', async ({
  request,
  browser,
}, testInfo) => {
  const member = await createMember(await adminApi(request, testInfo), testInfo, ip);
  const context = await freshContext(browser, testInfo, randomClientIp());
  const page = await context.newPage();
  await page.goto('/sign-in');

  const submit = page.getByTestId('signin-submit');
  const attempt = async () => {
    await page.getByTestId('signin-email-input').fill(member.email);
    await page.getByTestId('signin-password-input').fill('Wrong-Password-1!');
    await submit.click();
  };
  await attempt();
  await expect(page.getByTestId('signin-error')).toContainText(
    'That email and password do not match',
  );
  // The delay doubles after each failure, so a few quick failures reach a visible countdown.
  for (let index = 0; index < 4; index += 1) {
    if (await page.getByTestId('signin-throttle').isVisible()) {
      break;
    }
    await expect(submit).toBeEnabled();
    await attempt();
  }
  await expect(page.getByTestId('signin-throttle')).toContainText(/Try again in \d+ seconds?/);
  await expect(submit).toBeDisabled();

  // When the countdown ends the form works again.
  await expect(submit).toBeEnabled({ timeout: 20_000 });
  await context.close();
});
