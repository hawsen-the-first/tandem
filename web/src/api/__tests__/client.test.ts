import { describe, expect, it } from 'vitest';
import { mockApi } from '../../test/mockApi';
import { call, client } from '../client';
import { setCsrfToken } from '../csrf';
import { ApiError } from '../errors';
import { errorMessage, fieldErrors } from '../messages';

describe('API client', () => {
  it('sends the CSRF token on state-changing requests only', async () => {
    const api = mockApi({
      'GET /api/setup': { body: { available: true } },
      'POST /api/auth/sign-out': { status: 204 },
    });
    setCsrfToken('token-123');
    await call(client.GET('/api/setup'));
    await call(client.POST('/api/auth/sign-out'));
    expect(api.calls[0]?.headers.get('x-csrf-token')).toBeNull();
    expect(api.calls[1]?.headers.get('x-csrf-token')).toBe('token-123');
    setCsrfToken(null);
  });

  it('turns an error envelope into an ApiError with the retry time', async () => {
    mockApi({
      'POST /api/auth/sign-in': {
        status: 429,
        headers: { 'retry-after': '7' },
        body: { error: { code: 'rate_limited', message: 'Too many attempts.' } },
      },
    });
    const failure = await call(
      client.POST('/api/auth/sign-in', { body: { email: 'a@example.test', password: 'x' } }),
    ).catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(ApiError);
    expect((failure as ApiError).code).toBe('rate_limited');
    expect((failure as ApiError).retryAfterSeconds).toBe(7);
    expect(errorMessage(failure)).toBe('Too many attempts. Try again in 7 seconds');
  });

  it('reports a network failure with its own message', async () => {
    mockApi();
    const failure = await call(client.GET('/api/setup')).catch((error: unknown) => error);
    expect((failure as ApiError).code).toBe('network');
    expect(errorMessage(failure)).toContain('Could not reach Tandem');
  });

  it('treats a non-envelope server failure as a generic error', async () => {
    mockApi({ 'GET /api/setup': { status: 502, body: 'bad gateway' } });
    const failure = await call(client.GET('/api/setup')).catch((error: unknown) => error);
    expect((failure as ApiError).code).toBe('internal');
    expect(errorMessage(failure)).toBe('Something went wrong. Nothing was changed.');
  });
});

describe('error messages', () => {
  const make = (
    code: string,
    extra: Partial<{
      fields: { field: string; message: string }[];
      retryAfterSeconds: number;
    }> = {},
  ) => new ApiError({ status: 400, code, message: 'server text', ...extra });

  it('uses the wording from the business rules', () => {
    expect(errorMessage(make('invalid_credentials'))).toBe('That email and password do not match');
    expect(errorMessage(make('invalid_link'))).toBe(
      'This link is not valid. Ask an admin for a new one',
    );
    expect(errorMessage(make('forbidden'))).toBe('You do not have access');
    expect(errorMessage(make('not_found'))).toBe('Not found');
    expect(errorMessage(make('csrf'))).toContain('Reload the page');
    expect(errorMessage(make('rate_limited', { retryAfterSeconds: 1 }))).toBe(
      'Too many attempts. Try again in 1 second',
    );
  });

  it('never shows an unknown server message or a non-API error', () => {
    expect(errorMessage(make('something_new'))).toBe('Something went wrong. Nothing was changed.');
    expect(errorMessage(new Error('stack trace here'))).toBe(
      'Something went wrong. Nothing was changed.',
    );
  });

  it('prefers the first field problem for validation errors', () => {
    const error = make('validation', { fields: [{ field: 'password', message: 'Add a digit' }] });
    expect(errorMessage(error)).toBe('Add a digit');
    expect(fieldErrors(error)).toEqual({ password: 'Add a digit' });
    expect(fieldErrors(new Error('x'))).toEqual({});
  });
});
