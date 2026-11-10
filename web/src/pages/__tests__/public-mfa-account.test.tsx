import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it, vi } from 'vitest';
import { ME_ADMIN, ME_MEMBER, mockApi } from '../../test/mockApi';
import { renderApp } from '../../test/render';

const GOOD = 'Correct-Horse-7';
const ENROLMENT_ONLY = {
  ...ME_ADMIN,
  scope: 'mfa_enrolment_only',
  mfa: { ...ME_ADMIN.mfa, enrolled: false },
};
const ENROL = {
  body: {
    secret_base32: 'JBSWY3DPEHPK3PXP',
    otpauth_uri: 'otpauth://totp/Tandem:ada@example.test?secret=JBSWY3DPEHPK3PXP&issuer=Tandem',
  },
};
const CODES = { body: { recovery_codes: ['AAAA-1111', 'BBBB-2222'] } };

describe('MfaEnrolmentPage', () => {
  it('cannot be skipped by an admin', async () => {
    mockApi({ 'GET /api/auth/me': { body: ENROLMENT_ONLY } });
    const { container } = renderApp('/mfa/enrol');
    expect(await screen.findByTestId('mfa-required')).toBeInTheDocument();
    expect(screen.queryByTestId('mfa-skip')).not.toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('lets a member postpone it', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_MEMBER } });
    renderApp('/mfa/enrol');
    expect(await screen.findByTestId('mfa-skip')).toBeInTheDocument();
    expect(screen.queryByTestId('mfa-required')).not.toBeInTheDocument();
  });

  it('says so when two-step sign-in is already set up', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    renderApp('/mfa/enrol');
    expect(await screen.findByTestId('mfa-already')).toBeInTheDocument();
  });

  it('walks through scan, confirm and recovery codes, then finishes', async () => {
    const user = userEvent.setup();
    let me: unknown = ENROLMENT_ONLY;
    const api = mockApi({
      'GET /api/auth/me': () => ({ body: me }),
      'POST /api/auth/mfa/enrol': ENROL,
      'POST /api/auth/mfa/confirm': (request) => {
        const code = (request.body as { code: string }).code;
        if (code !== '123456') {
          return {
            status: 422,
            body: {
              error: {
                code: 'validation',
                message: 'x',
                fields: [{ field: 'code', message: 'That code is not right' }],
              },
            },
          };
        }
        me = ME_ADMIN;
        return CODES;
      },
    });
    const { container } = renderApp('/mfa/enrol');
    await user.click(await screen.findByTestId('mfa-start'));

    expect(await screen.findByTestId('mfa-scan')).toBeInTheDocument();
    const qr = await screen.findByTestId('mfa-qr');
    expect(qr.getAttribute('src')).toMatch(/^data:image\/svg\+xml/);
    expect(qr).toHaveAccessibleName(/QR code for your authenticator app/);
    expect(screen.getByTestId('mfa-secret')).toHaveTextContent('JBSWY3DPEHPK3PXP');
    expect(await axe(container)).toHaveNoViolations();
    await user.click(screen.getByTestId('mfa-next'));

    await user.click(screen.getByTestId('mfa-confirm'));
    expect(screen.getByTestId('mfa-code-error')).toHaveTextContent('Enter the 6-digit code');
    expect(screen.getByTestId('mfa-code-input')).toHaveFocus();
    await user.type(screen.getByTestId('mfa-code-input'), '000000');
    await user.click(screen.getByTestId('mfa-confirm'));
    expect(await screen.findByTestId('mfa-code-error')).toHaveTextContent('That code is not right');
    expect(screen.getByTestId('mfa-code-input')).toHaveValue('');

    await user.type(screen.getByTestId('mfa-code-input'), '123456');
    await user.click(screen.getByTestId('mfa-confirm'));
    expect(await screen.findByTestId('mfa-codes')).toBeInTheDocument();
    expect(screen.getByTestId('recovery-value')).toHaveTextContent('AAAA-1111');
    expect(await axe(container)).toHaveNoViolations();

    const done = screen.getByTestId('recovery-done');
    expect(done).toBeDisabled();
    await user.click(screen.getByTestId('recovery-ack'));
    await user.click(done);
    await waitFor(() => {
      expect(screen.queryByTestId('mfa-page')).not.toBeInTheDocument();
    });
    expect(api.calls.filter((call) => call.path === '/api/auth/me').length).toBeGreaterThan(1);
  });

  it('offers the recovery codes as a text file download', async () => {
    const user = userEvent.setup();
    const createObjectURL = vi.fn().mockReturnValue('blob:codes');
    const revokeObjectURL = vi.fn();
    Object.assign(URL, { createObjectURL, revokeObjectURL });
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => undefined);
    mockApi({
      'GET /api/auth/me': { body: ENROLMENT_ONLY },
      'POST /api/auth/mfa/enrol': ENROL,
      'POST /api/auth/mfa/confirm': CODES,
    });
    renderApp('/mfa/enrol');
    await user.click(await screen.findByTestId('mfa-start'));
    await user.click(await screen.findByTestId('mfa-next'));
    await user.type(screen.getByTestId('mfa-code-input'), '123456');
    await user.click(screen.getByTestId('mfa-confirm'));
    await user.click(await screen.findByTestId('mfa-download'));
    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(click).toHaveBeenCalledOnce();
    click.mockRestore();
  });

  it('shows a generic error when enrolment cannot start', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': { body: ENROLMENT_ONLY },
      'POST /api/auth/mfa/enrol': {
        status: 500,
        body: { error: { code: 'internal', message: 'x' } },
      },
    });
    renderApp('/mfa/enrol');
    await user.click(await screen.findByTestId('mfa-start'));
    expect(await screen.findByTestId('mfa-error')).toHaveTextContent('Something went wrong');
  });
});

describe('AccountPage', () => {
  const server = (me: unknown, extra: Record<string, never> = {}) =>
    mockApi({ 'GET /api/auth/me': { body: me }, ...extra });

  it('is accessible', async () => {
    server(ME_MEMBER);
    const { container } = renderApp('/account');
    await screen.findByTestId('account-page');
    expect(await axe(container)).toHaveNoViolations();
  });

  describe('change password', () => {
    it('validates before sending', async () => {
      const user = userEvent.setup();
      const api = server(ME_MEMBER);
      renderApp('/account');
      await screen.findByTestId('change-password-form');
      await user.click(screen.getByTestId('change-submit'));
      expect(screen.getByTestId('change-current-error')).toHaveTextContent(
        'Enter your current password',
      );
      expect(screen.getByTestId('change-new-error')).toBeInTheDocument();
      expect(screen.getByTestId('change-current-input')).toHaveFocus();
      expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
    });

    it('changes the password and says the other sign-ins were ended', async () => {
      const user = userEvent.setup();
      const api = mockApi({
        'GET /api/auth/me': { body: ME_MEMBER },
        'POST /api/auth/password': { status: 204 },
      });
      renderApp('/account');
      await screen.findByTestId('change-password-form');
      await user.type(screen.getByTestId('change-current-input'), 'Old-Password-1');
      await user.type(screen.getByTestId('change-new-input'), GOOD);
      await user.type(screen.getByTestId('change-confirm-input'), GOOD);
      await user.click(screen.getByTestId('change-submit'));
      expect(await screen.findByTestId('toast-success')).toHaveTextContent(
        'signed out of your other devices',
      );
      expect(api.calls.find((call) => call.path === '/api/auth/password')?.body).toEqual({
        current_password: 'Old-Password-1',
        new_password: GOOD,
      });
      expect(screen.getByTestId('change-new-input')).toHaveValue('');
    });

    it('shows a wrong current password next to its field and clears the form', async () => {
      const user = userEvent.setup();
      mockApi({
        'GET /api/auth/me': { body: ME_MEMBER },
        'POST /api/auth/password': {
          status: 422,
          body: {
            error: {
              code: 'validation',
              message: 'x',
              fields: [{ field: 'current_password', message: 'That is not your current password' }],
            },
          },
        },
      });
      renderApp('/account');
      await screen.findByTestId('change-password-form');
      await user.type(screen.getByTestId('change-current-input'), 'wrong');
      await user.type(screen.getByTestId('change-new-input'), GOOD);
      await user.type(screen.getByTestId('change-confirm-input'), GOOD);
      await user.click(screen.getByTestId('change-submit'));
      expect(await screen.findByTestId('change-current-error')).toHaveTextContent(
        'not your current password',
      );
      expect(screen.getByTestId('change-new-input')).toHaveValue('');
    });
  });

  describe('two-step sign-in', () => {
    it('offers set-up to a member who has not enrolled', async () => {
      server(ME_MEMBER);
      renderApp('/account');
      expect(await screen.findByTestId('mfa-status')).toHaveTextContent('Off');
      expect(screen.getByTestId('mfa-setup')).toHaveAttribute('href', '/mfa/enrol');
      expect(screen.queryByTestId('mfa-regenerate')).not.toBeInTheDocument();
      expect(screen.queryByTestId('mfa-disable')).not.toBeInTheDocument();
    });

    it('shows an admin that it is required, with no way to turn it off', async () => {
      server(ME_ADMIN);
      renderApp('/account');
      expect(await screen.findByTestId('mfa-required-note')).toBeInTheDocument();
      expect(screen.getByTestId('mfa-status')).toHaveTextContent('Unused recovery codes: 10');
      expect(screen.getByTestId('mfa-regenerate')).toBeInTheDocument();
      expect(screen.queryByTestId('mfa-disable')).not.toBeInTheDocument();
    });

    const ENROLLED_MEMBER = {
      ...ME_MEMBER,
      mfa: { enrolled: true, recovery_codes_remaining: 2, remind_regenerate: true },
    };

    it('reminds when recovery codes are running low', async () => {
      server(ENROLLED_MEMBER);
      renderApp('/account');
      expect(await screen.findByTestId('mfa-remind')).toBeInTheDocument();
    });

    it('replaces recovery codes only after confirmation and shows them once', async () => {
      const user = userEvent.setup();
      const api = mockApi({
        'GET /api/auth/me': { body: ENROLLED_MEMBER },
        'POST /api/auth/mfa/recovery-codes': CODES,
      });
      const { container } = renderApp('/account');
      await user.click(await screen.findByTestId('mfa-regenerate'));
      const dialog = screen.getByTestId('regenerate-dialog');
      expect(dialog).toHaveAttribute('open');
      await user.click(within(dialog).getByTestId('regenerate-dialog-cancel'));
      expect(api.calls.some((call) => call.path === '/api/auth/mfa/recovery-codes')).toBe(false);
      await user.click(screen.getByTestId('mfa-regenerate'));
      await user.click(screen.getByTestId('regenerate-dialog-confirm'));
      expect(await screen.findByTestId('regenerated-value')).toHaveTextContent('BBBB-2222');
      expect(await axe(container)).toHaveNoViolations();
      await user.click(screen.getByTestId('regenerated-ack'));
      await user.click(screen.getByTestId('regenerated-done'));
      expect(screen.queryByTestId('regenerated-value')).not.toBeInTheDocument();
    });

    it('turns two-step sign-in off with a password and a code', async () => {
      const user = userEvent.setup();
      let me: unknown = ENROLLED_MEMBER;
      const api = mockApi({
        'GET /api/auth/me': () => ({ body: me }),
        'POST /api/auth/mfa/disable': () => {
          me = ME_MEMBER;
          return { status: 204 };
        },
      });
      const { container } = renderApp('/account');
      await user.click(await screen.findByTestId('mfa-disable'));
      expect(await axe(container)).toHaveNoViolations();
      await user.click(screen.getByTestId('disable-submit'));
      expect(screen.getByTestId('disable-password-error')).toBeInTheDocument();
      expect(screen.getByTestId('disable-code-error')).toBeInTheDocument();
      await user.type(screen.getByTestId('disable-password-input'), GOOD);
      await user.type(screen.getByTestId('disable-code-input'), '123456');
      await user.click(screen.getByTestId('disable-submit'));
      expect(await screen.findByTestId('toast-success')).toHaveTextContent(
        'Two-step sign-in is off',
      );
      expect(api.calls.find((call) => call.path === '/api/auth/mfa/disable')?.body).toEqual({
        password: GOOD,
        code: '123456',
      });
      await waitFor(() => {
        expect(screen.getByTestId('mfa-status')).toHaveTextContent('Off');
      });
    });

    it('shows a rejected password or code and clears both', async () => {
      const user = userEvent.setup();
      mockApi({
        'GET /api/auth/me': { body: ENROLLED_MEMBER },
        'POST /api/auth/mfa/disable': {
          status: 422,
          body: {
            error: {
              code: 'validation',
              message: 'x',
              fields: [{ field: 'code', message: 'That code is not right' }],
            },
          },
        },
      });
      renderApp('/account');
      await user.click(await screen.findByTestId('mfa-disable'));
      await user.type(screen.getByTestId('disable-password-input'), GOOD);
      await user.type(screen.getByTestId('disable-code-input'), '000000');
      await user.click(screen.getByTestId('disable-submit'));
      expect(await screen.findByTestId('disable-code-error')).toHaveTextContent('not right');
      expect(screen.getByTestId('disable-password-input')).toHaveValue('');
    });
  });
});
