import createClient, { type Middleware } from 'openapi-fetch';
import { getCsrfToken } from './csrf';
import { ApiError, toApiError } from './errors';
import type { paths } from './schema';

const SAFE_METHODS = new Set(['GET', 'HEAD', 'OPTIONS']);

const csrf: Middleware = {
  onRequest({ request }) {
    const token = getCsrfToken();
    if (!SAFE_METHODS.has(request.method) && token !== null) {
      request.headers.set('X-CSRF-Token', token);
    }
    return request;
  },
};

/** The typed client. `fetch` is looked up per call so tests can replace it. */
export const client = createClient<paths>({
  baseUrl: window.location.origin,
  credentials: 'same-origin',
  fetch: (request) => globalThis.fetch(request),
});
client.use(csrf);

interface Outcome<T> {
  data?: T | undefined;
  error?: unknown;
  response: Response;
}

/**
 * Await a client call and return its data, or throw an ApiError built from the server's error
 * envelope. Calls with no body (204) resolve to `undefined`.
 */
export async function call<T>(request: Promise<Outcome<T>>): Promise<T> {
  let outcome: Outcome<T>;
  try {
    outcome = await request;
  } catch {
    throw new ApiError({
      status: 0,
      code: 'network',
      message: 'Could not reach Tandem. Check your connection and try again.',
    });
  }
  if (outcome.response.ok) {
    return outcome.data as T;
  }
  throw toApiError(outcome.response.status, outcome.error, outcome.response.headers);
}
