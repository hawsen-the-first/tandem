import { vi } from 'vitest';

export interface MockRequest {
  method: string;
  path: string;
  search: string;
  body: unknown;
  headers: Headers;
}

export interface MockReply {
  status?: number;
  body?: unknown;
  headers?: Record<string, string>;
}

export type MockHandler = MockReply | ((request: MockRequest) => MockReply | Promise<MockReply>);

export interface MockApi {
  /** Every request seen, in order. */
  calls: MockRequest[];
  /** Replace or add a handler for "METHOD /path". */
  on: (route: string, handler: MockHandler) => void;
}

/**
 * Replace `fetch` with an in-memory API. Routes are "METHOD /path" (no query string). A request
 * with no handler fails the test with a clear message instead of reaching the network.
 */
export function mockApi(initial: Record<string, MockHandler> = {}): MockApi {
  const handlers = new Map<string, MockHandler>(Object.entries(initial));
  const calls: MockRequest[] = [];
  vi.stubGlobal('fetch', async (input: Request | string, init?: RequestInit) => {
    const request =
      typeof input === 'string' ? new Request(new URL(input, 'http://localhost'), init) : input;
    const url = new URL(request.url, 'http://localhost');
    const text =
      request.method === 'GET' || request.method === 'HEAD' ? '' : await request.clone().text();
    let body: unknown = undefined;
    if (text !== '') {
      try {
        body = JSON.parse(text);
      } catch {
        body = text;
      }
    }
    const seen: MockRequest = {
      method: request.method,
      path: url.pathname,
      search: url.search,
      body,
      headers: request.headers,
    };
    calls.push(seen);
    const handler = handlers.get(`${request.method} ${url.pathname}`);
    if (handler === undefined) {
      throw new Error(`mockApi: no handler for ${request.method} ${url.pathname}`);
    }
    const reply = typeof handler === 'function' ? await handler(seen) : handler;
    const status = reply.status ?? 200;
    const noBody = status === 204 || reply.body === undefined;
    return new Response(noBody ? null : JSON.stringify(reply.body), {
      status,
      headers: { ...(noBody ? {} : { 'content-type': 'application/json' }), ...reply.headers },
    });
  });
  return {
    calls,
    on: (route, handler) => {
      handlers.set(route, handler);
    },
  };
}

export const ME_ADMIN = {
  user: {
    id: '019b0000-0000-7000-8000-000000000001',
    email: 'ada@example.test',
    display_name: 'Ada Admin',
    role: 'admin',
    status: 'active',
    created_at: '2026-01-01T00:00:00Z',
    last_sign_in_at: null,
  },
  scope: 'full',
  mfa: { enrolled: true, recovery_codes_remaining: 10, remind_regenerate: false },
  csrf_token: 'csrf-admin',
};

export const ME_MEMBER = {
  user: {
    ...ME_ADMIN.user,
    id: '019b0000-0000-7000-8000-000000000002',
    email: 'mia@example.test',
    display_name: 'Mia Member',
    role: 'member',
  },
  scope: 'full',
  mfa: { enrolled: false, recovery_codes_remaining: 0, remind_regenerate: false },
  csrf_token: 'csrf-member',
};

export const UNAUTHENTICATED = {
  status: 401,
  body: { error: { code: 'unauthenticated', message: 'You need to sign in.' } },
};
