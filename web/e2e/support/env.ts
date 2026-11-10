import { randomBytes } from 'node:crypto';
import type { APIRequestContext, Browser, BrowserContext, Page, TestInfo } from '@playwright/test';
import { request as playwrightRequest } from '@playwright/test';

export const PAGE_ORIGIN = (testInfo: TestInfo): string =>
  new URL(String(testInfo.project.use.baseURL)).origin;

/**
 * The stack runs with 127.0.0.1 as a trusted proxy, so each spec file presents its own client
 * address through X-Forwarded-For. That keeps one file's sign-ins from using up another file's
 * per-source allowance, and it exercises the proxy path.
 */
export function randomClientIp(): string {
  const bytes = randomBytes(3);
  return `10.${String(bytes[0])}.${String(bytes[1])}.${String(1 + ((bytes[2] ?? 0) % 250))}`;
}

export function clientHeaders(ip: string): Record<string, string> {
  return { 'x-forwarded-for': ip };
}

export function strongPassword(): string {
  return `Tandem-${randomBytes(6).toString('hex')}-Zq9!`;
}

export function uniqueEmail(prefix: string): string {
  return `${prefix}-${randomBytes(5).toString('hex')}@example.test`;
}

/** A browser context with no stored session, presenting the given client address. */
export function freshContext(
  browser: Browser,
  testInfo: TestInfo,
  ip: string,
  extra: { viewport?: { width: number; height: number } } = {},
): Promise<BrowserContext> {
  return browser.newContext({
    baseURL: String(testInfo.project.use.baseURL),
    extraHTTPHeaders: clientHeaders(ip),
    storageState: { cookies: [], origins: [] },
    ...extra,
  });
}

export interface AdminApi {
  get: (path: string) => Promise<unknown>;
  post: (path: string, data?: unknown) => Promise<unknown>;
  put: (path: string, data?: unknown) => Promise<unknown>;
}

/** Calls the admin API with the admin session of the current project (cookie, CSRF token, Origin). */
export async function adminApi(request: APIRequestContext, testInfo: TestInfo): Promise<AdminApi> {
  const origin = PAGE_ORIGIN(testInfo);
  const me = (await (await request.get('/api/auth/me')).json()) as { csrf_token: string };
  const headers = { 'X-CSRF-Token': me.csrf_token, Origin: origin };
  const read = async (response: Awaited<ReturnType<APIRequestContext['get']>>) => {
    if (!response.ok()) {
      throw new Error(`${response.url()} answered ${String(response.status())}`);
    }
    return response.status() === 204 ? null : await response.json();
  };
  return {
    get: async (path) => read(await request.get(path)),
    post: async (path, data) => read(await request.post(path, { headers, data })),
    put: async (path, data) => read(await request.put(path, { headers, data })),
  };
}

export interface Member {
  email: string;
  password: string;
  displayName: string;
  id: string;
}

/** Create a member through the API: invite as admin, accept on a clean connection. */
export async function createMember(
  admin: AdminApi,
  testInfo: TestInfo,
  ip: string,
  displayName = 'Mia Member',
): Promise<Member> {
  const email = uniqueEmail('member');
  const password = strongPassword();
  const invite = (await admin.post('/api/admin/invitations', { email, role: 'member' })) as {
    link: string;
  };
  const token = new URL(invite.link).searchParams.get('token') ?? '';
  const anonymous = await playwrightRequest.newContext({
    baseURL: String(testInfo.project.use.baseURL),
    extraHTTPHeaders: clientHeaders(ip),
  });
  const accepted = await anonymous.post('/api/invitations/accept', {
    headers: { Origin: PAGE_ORIGIN(testInfo) },
    data: { token, display_name: displayName, password },
  });
  if (!accepted.ok()) {
    throw new Error(`accepting the invitation answered ${String(accepted.status())}`);
  }
  const body = (await accepted.json()) as { user: { id: string } };
  await anonymous.dispose();
  return { email, password, displayName, id: body.user.id };
}

export async function signInViaUi(page: Page, email: string, password: string): Promise<void> {
  await page.goto('/sign-in');
  await page.getByTestId('signin-email-input').fill(email);
  await page.getByTestId('signin-password-input').fill(password);
  await page.getByTestId('signin-submit').click();
}

/** Sign in through the form and wait until the signed-in shell is showing. */
export async function signedInViaUi(page: Page, email: string, password: string): Promise<void> {
  await signInViaUi(page, email, password);
  await page.getByRole('navigation', { name: 'Main navigation' }).waitFor();
}

/**
 * A failed sign-in starts a short delay (1 s, then doubling) for that account, so a person who
 * fails and immediately retries is told to wait. Wait it out before the next attempt.
 */
export async function waitOutSignInDelay(page: Page): Promise<void> {
  await page.waitForTimeout(1500);
}
