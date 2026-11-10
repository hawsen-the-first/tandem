import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { ME_MEMBER, UNAUTHENTICATED, mockApi } from '../../test/mockApi';
import { renderApp } from '../../test/render';

const GOOD = 'Correct-Horse-7';
const INVALID_LINK = {
  status: 404,
  body: { error: { code: 'invalid_link', message: 'This link is not valid.' } },
};

describe('AcceptInvitationPage', () => {
  it('says the link is not valid when it is, with one generic message', async () => {
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/invitations/validate': INVALID_LINK,
    });
    const { container } = renderApp('/accept-invitation?token=old');
    expect(await screen.findByTestId('invite-invalid')).toHaveTextContent(
      'This link is not valid. Ask an admin for a new one',
    );
    expect(await axe(container)).toHaveNoViolations();
  });

  it('treats a missing token as an invalid link without calling the API', async () => {
    const api = mockApi({ 'GET /api/auth/me': UNAUTHENTICATED });
    renderApp('/accept-invitation');
    expect(await screen.findByTestId('invite-invalid')).toBeInTheDocument();
    expect(api.calls.some((call) => call.path === '/api/invitations/validate')).toBe(false);
  });

  it('validates the link, shows who it is for and is accessible', async () => {
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/invitations/validate': { body: { email: 'mia@example.test', role: 'member' } },
    });
    const { container } = renderApp('/accept-invitation?token=abc123');
    expect(await screen.findByTestId('invite-page')).toHaveTextContent('mia@example.test');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('checks the form, then creates the account and signs in', async () => {
    const user = userEvent.setup();
    let signedIn = false;
    const api = mockApi({
      'GET /api/auth/me': () => (signedIn ? { body: ME_MEMBER } : UNAUTHENTICATED),
      'POST /api/invitations/validate': { body: { email: 'mia@example.test', role: 'member' } },
      'POST /api/invitations/accept': () => {
        signedIn = true;
        return { body: { user: ME_MEMBER.user, scope: 'full' } };
      },
    });
    renderApp('/accept-invitation?token=abc123');
    await screen.findByTestId('invite-page');
    await user.click(screen.getByTestId('invite-submit'));
    expect(screen.getByTestId('invite-name-error')).toBeInTheDocument();
    expect(screen.getByTestId('invite-name-input')).toHaveFocus();
    await user.type(screen.getByTestId('invite-name-input'), 'Mia Member');
    await user.type(screen.getByTestId('invite-password-input'), GOOD);
    await user.type(screen.getByTestId('invite-confirm-input'), GOOD);
    await user.click(screen.getByTestId('invite-submit'));
    await waitFor(() => {
      expect(screen.queryByTestId('invite-page')).not.toBeInTheDocument();
    });
    const accept = api.calls.find((call) => call.path === '/api/invitations/accept');
    expect(accept?.body).toEqual({ token: 'abc123', display_name: 'Mia Member', password: GOOD });
    expect(api.calls.filter((call) => call.path === '/api/invitations/validate')).toHaveLength(1);
  });

  it('shows a server error and clears the passwords', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/invitations/validate': { body: { email: 'mia@example.test', role: 'member' } },
      'POST /api/invitations/accept': INVALID_LINK,
    });
    renderApp('/accept-invitation?token=abc123');
    await screen.findByTestId('invite-page');
    await user.type(screen.getByTestId('invite-name-input'), 'Mia');
    await user.type(screen.getByTestId('invite-password-input'), GOOD);
    await user.type(screen.getByTestId('invite-confirm-input'), GOOD);
    await user.click(screen.getByTestId('invite-submit'));
    expect(await screen.findByTestId('invite-error')).toHaveTextContent('This link is not valid');
    expect(screen.getByTestId('invite-password-input')).toHaveValue('');
  });
});

describe('ResetPasswordPage', () => {
  it('rejects an invalid link', async () => {
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/password-resets/validate': INVALID_LINK,
    });
    const { container } = renderApp('/reset-password?token=old');
    expect(await screen.findByTestId('reset-invalid')).toHaveTextContent('This link is not valid');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('checks the new password and the confirmation before sending', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/password-resets/validate': { body: { display_name: 'Mia Member' } },
    });
    const { container } = renderApp('/reset-password?token=tok');
    expect(await screen.findByTestId('reset-page')).toHaveTextContent('Mia Member');
    expect(await axe(container)).toHaveNoViolations();
    await user.click(screen.getByTestId('reset-submit'));
    expect(screen.getByTestId('reset-password-error')).toBeInTheDocument();
    expect(screen.getByTestId('reset-password-input')).toHaveFocus();
    await user.type(screen.getByTestId('reset-password-input'), GOOD);
    await user.type(screen.getByTestId('reset-confirm-input'), 'nope');
    await user.click(screen.getByTestId('reset-submit'));
    expect(screen.getByTestId('reset-confirm-error')).toHaveTextContent('Passwords do not match');
    expect(api.calls.some((call) => call.path === '/api/password-resets/complete')).toBe(false);
  });

  it('completes the reset and sends the person to sign-in with a notice', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/password-resets/validate': { body: { display_name: 'Mia Member' } },
      'POST /api/password-resets/complete': { status: 204 },
    });
    renderApp('/reset-password?token=tok');
    await screen.findByTestId('reset-page');
    await user.type(screen.getByTestId('reset-password-input'), GOOD);
    await user.type(screen.getByTestId('reset-confirm-input'), GOOD);
    await user.click(screen.getByTestId('reset-submit'));
    expect(await screen.findByTestId('signin-notice')).toHaveTextContent(
      'Your password was changed. Sign in with the new password.',
    );
    expect(api.calls.find((call) => call.path === '/api/password-resets/complete')?.body).toEqual({
      token: 'tok',
      new_password: GOOD,
    });
  });

  it('shows a server rejection of the new password next to the field', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': UNAUTHENTICATED,
      'POST /api/password-resets/validate': { body: { display_name: 'Mia' } },
      'POST /api/password-resets/complete': {
        status: 422,
        body: {
          error: {
            code: 'validation',
            message: 'Check',
            fields: [{ field: 'password', message: 'Too common' }],
          },
        },
      },
    });
    renderApp('/reset-password?token=tok');
    await screen.findByTestId('reset-page');
    await user.type(screen.getByTestId('reset-password-input'), GOOD);
    await user.type(screen.getByTestId('reset-confirm-input'), GOOD);
    await user.click(screen.getByTestId('reset-submit'));
    expect(await screen.findByTestId('reset-password-error')).toHaveTextContent('Too common');
  });
});
