import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ME_ADMIN, UNAUTHENTICATED, mockApi } from '../../test/mockApi';
import { renderApp } from '../../test/render';

const GOOD = 'Correct-Horse-7';
const ENROLMENT_ONLY = {
  ...ME_ADMIN,
  scope: 'mfa_enrolment_only',
  mfa: { ...ME_ADMIN.mfa, enrolled: false },
};

afterEach(() => {
  vi.useRealTimers();
});

/** `me` is anonymous until `signedIn.current` is set, like the real server. */
function session(signedIn: { current: unknown }) {
  return () => (signedIn.current === null ? UNAUTHENTICATED : { body: signedIn.current });
}

describe('SetupPage', () => {
  it('shows "not found" once an admin exists', async () => {
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'GET /api/setup': { body: { available: false } },
    });
    renderApp('/setup');
    expect(await screen.findByTestId('not-found-page')).toBeInTheDocument();
  });

  it('is accessible and shows the live password checklist', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'GET /api/setup': { body: { available: true } },
    });
    const { container } = renderApp('/setup');
    await screen.findByTestId('setup-page');
    expect(await axe(container)).toHaveNoViolations();
    expect(screen.getByTestId('setup-password-rule-digit')).toHaveAttribute('data-met', 'false');
    await user.type(screen.getByTestId('setup-password-input'), 'Abc1');
    expect(screen.getByTestId('setup-password-rule-digit')).toHaveAttribute('data-met', 'true');
  });

  it('reports every client-side problem, focuses the first and sends nothing', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'GET /api/setup': { body: { available: true } },
    });
    const { container } = renderApp('/setup');
    await screen.findByTestId('setup-page');
    await user.click(screen.getByTestId('setup-submit'));
    expect(screen.getByTestId('setup-name-error')).toHaveTextContent(
      'Enter a name up to 80 characters',
    );
    expect(screen.getByTestId('setup-email-error')).toHaveTextContent(
      'Enter a valid email address',
    );
    expect(screen.getByTestId('setup-password-error')).toBeInTheDocument();
    expect(screen.getByTestId('setup-name-input')).toHaveFocus();
    expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
    expect(await axe(container)).toHaveNoViolations();
  });

  it('requires the confirmation to match', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'GET /api/setup': { body: { available: true } },
    });
    renderApp('/setup');
    await screen.findByTestId('setup-page');
    await user.type(screen.getByTestId('setup-name-input'), 'Ada');
    await user.type(screen.getByTestId('setup-email-input'), 'ada@example.test');
    await user.type(screen.getByTestId('setup-password-input'), GOOD);
    await user.type(screen.getByTestId('setup-confirm-input'), 'different');
    await user.click(screen.getByTestId('setup-submit'));
    expect(screen.getByTestId('setup-confirm-error')).toHaveTextContent('Passwords do not match');
  });

  it('creates the admin and continues to MFA enrolment', async () => {
    const user = userEvent.setup();
    const signedIn: { current: unknown } = { current: null };
    const api = mockApi({
      'GET /api/auth/me': session(signedIn),
      'GET /api/setup': { body: { available: true } },
      'POST /api/setup': () => {
        signedIn.current = ENROLMENT_ONLY;
        return { body: { user: ME_ADMIN.user, scope: 'mfa_enrolment_only' } };
      },
    });
    renderApp('/setup');
    await screen.findByTestId('setup-page');
    await user.type(screen.getByTestId('setup-name-input'), '  Ada Admin ');
    await user.type(screen.getByTestId('setup-email-input'), 'ada@example.test');
    await user.type(screen.getByTestId('setup-password-input'), GOOD);
    await user.type(screen.getByTestId('setup-confirm-input'), GOOD);
    await user.click(screen.getByTestId('setup-submit'));
    expect(await screen.findByTestId('mfa-page')).toBeInTheDocument();
    const post = api.calls.find((call) => call.method === 'POST');
    expect(post?.body).toEqual({
      display_name: 'Ada Admin',
      email: 'ada@example.test',
      password: GOOD,
    });
  });

  it('shows a server field error next to its field and clears the passwords', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'GET /api/setup': { body: { available: true } },
      'POST /api/setup': {
        status: 422,
        body: {
          error: {
            code: 'validation',
            message: 'Check',
            fields: [
              {
                field: 'password',
                message: 'That password appears in a list of breached passwords',
              },
            ],
          },
        },
      },
    });
    renderApp('/setup');
    await screen.findByTestId('setup-page');
    await user.type(screen.getByTestId('setup-name-input'), 'Ada');
    await user.type(screen.getByTestId('setup-email-input'), 'ada@example.test');
    await user.type(screen.getByTestId('setup-password-input'), GOOD);
    await user.type(screen.getByTestId('setup-confirm-input'), GOOD);
    await user.click(screen.getByTestId('setup-submit'));
    expect(await screen.findByTestId('setup-password-error')).toHaveTextContent('breached');
    expect(screen.getByTestId('setup-password-input')).toHaveValue('');
    expect(screen.getByTestId('setup-confirm-input')).toHaveValue('');
    await waitFor(() => {
      expect(screen.getByTestId('setup-password-input')).toHaveFocus();
    });
  });
});

describe('SignInPage', () => {
  async function fill(user: ReturnType<typeof userEvent.setup>) {
    await user.type(screen.getByTestId('signin-email-input'), 'ada@example.test');
    await user.type(screen.getByTestId('signin-password-input'), GOOD);
  }

  it('is accessible and validates before sending', async () => {
    const user = userEvent.setup();
    const api = mockApi({ 'GET /api/auth/me': UNAUTHENTICATED });
    const { container } = renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    expect(await axe(container)).toHaveNoViolations();
    await user.click(screen.getByTestId('signin-submit'));
    expect(screen.getByTestId('signin-email-error')).toHaveTextContent(
      'Enter a valid email address',
    );
    expect(screen.getByTestId('signin-password-error')).toHaveTextContent('Enter your password');
    expect(screen.getByTestId('signin-email-input')).toHaveFocus();
    expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
  });

  it('submits with Enter from the keyboard and leaves the sign-in page', async () => {
    const user = userEvent.setup();
    const signedIn: { current: unknown } = { current: null };
    const api = mockApi({
      'GET /api/auth/me': session(signedIn),
      'POST /api/auth/sign-in': () => {
        signedIn.current = ME_ADMIN;
        return { body: { status: 'signed_in', user: ME_ADMIN.user, scope: 'full' } };
      },
    });
    renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    await user.type(screen.getByTestId('signin-email-input'), 'ada@example.test');
    await user.tab();
    expect(screen.getByTestId('signin-password-input')).toHaveFocus();
    await user.keyboard(`${GOOD}{Enter}`);
    await waitFor(() => {
      expect(screen.queryByTestId('signin-page')).not.toBeInTheDocument();
    });
    expect(api.calls.find((call) => call.method === 'POST')?.body).toEqual({
      email: 'ada@example.test',
      password: GOOD,
      second_factor: null,
    });
  });

  it('gives one generic message for bad credentials and drops the password', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/auth/sign-in': {
        status: 401,
        body: { error: { code: 'invalid_credentials', message: 'x' } },
      },
    });
    const { container } = renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    await fill(user);
    await user.click(screen.getByTestId('signin-submit'));
    expect(await screen.findByTestId('signin-error')).toHaveTextContent(
      'That email and password do not match',
    );
    expect(screen.getByTestId('signin-password-input')).toHaveValue('');
    await waitFor(() => {
      expect(screen.getByTestId('signin-error')).toHaveFocus();
    });
    expect(await axe(container)).toHaveNoViolations();
  });

  it('asks for the second factor and sends it with the credentials', async () => {
    const user = userEvent.setup();
    const signedIn: { current: unknown } = { current: null };
    const api = mockApi({
      'GET /api/auth/me': session(signedIn),
      'POST /api/auth/sign-in': (request) => {
        const body = request.body as { second_factor: { kind: string } | null };
        if (body.second_factor === null) {
          return { body: { status: 'mfa_required' } };
        }
        signedIn.current = ME_ADMIN;
        return { body: { status: 'signed_in', user: ME_ADMIN.user, scope: 'full' } };
      },
    });
    const { container } = renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    await fill(user);
    await user.click(screen.getByTestId('signin-submit'));
    expect(await screen.findByTestId('mfa-form')).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
    await user.click(screen.getByTestId('mfa-submit'));
    expect(screen.getByTestId('mfa-code-error')).toHaveTextContent('Enter the 6-digit code');
    await user.type(screen.getByTestId('mfa-code-input'), '123456');
    await user.click(screen.getByTestId('mfa-submit'));
    await waitFor(() => {
      expect(screen.queryByTestId('signin-page')).not.toBeInTheDocument();
    });
    const posts = api.calls.filter((call) => call.method === 'POST');
    expect(posts[1]?.body).toEqual({
      email: 'ada@example.test',
      password: GOOD,
      second_factor: { kind: 'totp', code: '123456' },
    });
  });

  it('can switch to a recovery code and back', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/auth/sign-in': (request) => {
        const body = request.body as { second_factor: unknown };
        return body.second_factor === null
          ? { body: { status: 'mfa_required' } }
          : { status: 401, body: { error: { code: 'invalid_credentials', message: 'x' } } };
      },
    });
    renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    await fill(user);
    await user.click(screen.getByTestId('signin-submit'));
    await user.click(await screen.findByTestId('mfa-toggle-recovery'));
    expect(screen.getByLabelText(/Recovery code/)).toBeInTheDocument();
    await user.click(screen.getByTestId('mfa-submit'));
    expect(screen.getByTestId('mfa-code-error')).toHaveTextContent(
      'Enter one of your recovery codes',
    );
    await user.type(screen.getByTestId('mfa-code-input'), 'AAAA-BBBB');
    await user.click(screen.getByTestId('mfa-submit'));
    expect(await screen.findByTestId('signin-error')).toHaveTextContent('do not match');
    expect(api.calls.filter((call) => call.method === 'POST')[1]?.body).toMatchObject({
      second_factor: { kind: 'recovery', code: 'AAAA-BBBB' },
    });
    expect(screen.getByTestId('signin-email-input')).toBeInTheDocument();
  });

  it('starts over from the second step', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/auth/sign-in': { body: { status: 'mfa_required' } },
    });
    renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    await fill(user);
    await user.click(screen.getByTestId('signin-submit'));
    await user.click(await screen.findByTestId('mfa-back'));
    expect(screen.getByTestId('signin-password-input')).toHaveValue('');
  });

  it('counts down a throttle and re-enables sign-in when it ends', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime.bind(vi) });
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/auth/sign-in': {
        status: 429,
        headers: { 'retry-after': '3' },
        body: { error: { code: 'rate_limited', message: 'x' } },
      },
    });
    const { container } = renderApp('/sign-in');
    await screen.findByTestId('signin-page');
    await fill(user);
    await user.click(screen.getByTestId('signin-submit'));
    const throttle = await screen.findByTestId('signin-throttle');
    expect(throttle).toHaveTextContent('Too many attempts. Try again in 3 seconds');
    expect(screen.getByTestId('signin-submit')).toBeDisabled();
    expect(await axe(container)).toHaveNoViolations();
    act(() => {
      vi.advanceTimersByTime(1000);
    });
    expect(screen.getByTestId('signin-throttle')).toHaveTextContent('Try again in 2 seconds');
    act(() => {
      vi.advanceTimersByTime(2500);
    });
    await waitFor(() => {
      expect(screen.queryByTestId('signin-throttle')).not.toBeInTheDocument();
    });
    expect(screen.getByTestId('signin-submit')).toBeEnabled();
  });

  it('sends a signed-in visitor away from the sign-in page', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    renderApp('/sign-in');
    await waitFor(() => {
      expect(screen.queryByTestId('signin-page')).not.toBeInTheDocument();
    });
    expect(await screen.findByRole('navigation', { name: 'Main navigation' })).toBeInTheDocument();
  });
});
