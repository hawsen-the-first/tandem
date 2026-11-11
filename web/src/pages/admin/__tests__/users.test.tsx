import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../../test/render';
import { UserTable } from '../users/UserTable';
import { activeAdminCount, mergeRows } from '../users/types';
import { ACCEPTED, ADA, BO, DEE, LAST_ADMIN_ERROR, MIA, PENDING, adminApi } from './fixtures';

const noop = () => undefined;

async function openPage() {
  const view = renderApp('/admin/users');
  await screen.findByTestId('users-table');
  return view;
}

function rowFor(text: string): HTMLElement {
  const row = screen
    .getAllByTestId('users-table-row')
    .find((candidate) => candidate.textContent.includes(text));
  if (row === undefined) {
    throw new Error(`no row contains ${text}`);
  }
  return row;
}

describe('merging users and invitations', () => {
  it('lists users first and only pending invitations after them', () => {
    const rows = mergeRows([ADA, MIA], [PENDING, ACCEPTED]);
    expect(rows.map((row) => row.kind)).toEqual(['user', 'user', 'invitation']);
  });

  it('counts only active admins', () => {
    expect(activeAdminCount([ADA, BO, MIA, DEE])).toBe(2);
    expect(activeAdminCount([ADA, { ...BO, status: 'deactivated' }])).toBe(1);
  });
});

describe('UserTable', () => {
  const props = {
    onChangeRole: noop,
    onResetLink: noop,
    onDeactivate: noop,
    onReactivate: noop,
    onRevoke: noop,
  };

  it('disables actions that would remove the last active admin, and says why', async () => {
    const users = [BO, MIA];
    const { container } = render(
      <UserTable rows={mergeRows(users, [])} users={users} currentUserId={ADA.id} {...props} />,
    );
    expect(screen.getByRole('button', { name: 'Deactivate Bo Builder' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Change role for Bo Builder' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Deactivate Mia Member' })).toBeEnabled();
    expect(
      screen.getByRole('button', { name: 'Deactivate Bo Builder' }),
    ).toHaveAccessibleDescription('At least one active admin is required');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('does not offer actions on your own account', () => {
    const users = [ADA, BO];
    render(
      <UserTable rows={mergeRows(users, [])} users={users} currentUserId={ADA.id} {...props} />,
    );
    expect(screen.getByRole('button', { name: 'Deactivate Ada Admin' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Issue a reset link for Ada Admin' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Change role for Ada Admin' })).toBeDisabled();
    expect(screen.getByText('Not available for your own account')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Deactivate Bo Builder' })).toBeEnabled();
  });

  it('offers reactivation, not deactivation, for a deactivated user', () => {
    render(
      <UserTable rows={mergeRows([DEE], [])} users={[DEE]} currentUserId={ADA.id} {...props} />,
    );
    expect(screen.getByRole('button', { name: 'Reactivate Dee Dormant' })).toBeEnabled();
    expect(
      screen.queryByRole('button', { name: 'Deactivate Dee Dormant' }),
    ).not.toBeInTheDocument();
  });
});

describe('UsersPage', () => {
  it('shows users and pending invitations with their status', async () => {
    adminApi();
    const { container } = await openPage();
    expect(screen.getAllByTestId('users-table-row')).toHaveLength(5);
    expect(within(rowFor('Ada Admin')).getByTestId('users-status')).toHaveTextContent('Active');
    expect(within(rowFor('Ada Admin')).getByText('(you)', { exact: false })).toBeInTheDocument();
    expect(within(rowFor('Dee Dormant')).getByTestId('users-status')).toHaveTextContent(
      'Deactivated',
    );
    expect(within(rowFor('new@example.test')).getByTestId('users-status')).toHaveTextContent(
      'Pending invitation',
    );
    expect(screen.queryByText('old@example.test')).not.toBeInTheDocument();
    expect(within(rowFor('Bo Builder')).getByText('Never')).toBeInTheDocument();
    const time = within(rowFor('Mia Member')).getByText(
      (_, element) => element?.tagName === 'TIME',
    );
    expect(time).toHaveAttribute('datetime', '2026-03-01T08:00:00Z');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows a loading state, then the table', async () => {
    adminApi({
      'GET /api/admin/users': async () => {
        await new Promise((resolve) => setTimeout(resolve, 30));
        return { body: [ADA] };
      },
    });
    renderApp('/admin/users');
    expect(await screen.findByTestId('users-loading')).toHaveAttribute('role', 'status');
    expect(await screen.findByTestId('users-table')).toBeInTheDocument();
  });

  it('shows an error with a retry that recovers', async () => {
    const user = userEvent.setup();
    const api = adminApi({ 'GET /api/admin/users': { status: 500, body: 'boom' } });
    const { container } = renderApp('/admin/users');
    const error = await screen.findByTestId('users-error');
    expect(error).toHaveTextContent('Could not load the users');
    expect(await axe(container)).toHaveNoViolations();
    api.on('GET /api/admin/users', { body: [ADA] });
    await user.click(within(error).getByRole('button', { name: 'Try again' }));
    expect(await screen.findByTestId('users-table')).toBeInTheDocument();
  });

  it('shows an empty state when there is nobody', async () => {
    adminApi({ 'GET /api/admin/invitations': { body: [] } }, []);
    renderApp('/admin/users');
    expect(await screen.findByTestId('users-table-empty')).toHaveTextContent('No users yet');
  });

  describe('inviting', () => {
    it('validates the email before calling the server', async () => {
      const user = userEvent.setup();
      const api = adminApi();
      await openPage();
      await user.click(screen.getByTestId('users-invite'));
      await user.type(screen.getByTestId('invite-email-input'), 'not-an-email');
      await user.click(screen.getByTestId('invite-dialog-confirm'));
      expect(screen.getByTestId('invite-email-error')).toHaveTextContent(
        'Enter a valid email address',
      );
      expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
    });

    it('shows the link once, makes the person acknowledge it, and clears it on dismissal', async () => {
      const user = userEvent.setup();
      const api = adminApi({
        'POST /api/admin/invitations': {
          status: 201,
          body: {
            invitation: { ...PENDING, email: 'zed@example.test' },
            link: 'https://tandem.example.test/accept-invitation?token=abc123',
          },
        },
      });
      await openPage();
      await user.click(screen.getByTestId('users-invite'));
      await user.type(screen.getByTestId('invite-email-input'), 'zed@example.test');
      await user.selectOptions(screen.getByTestId('invite-role-input'), 'admin');
      await user.click(screen.getByTestId('invite-dialog-confirm'));

      const secret = await screen.findByTestId('users-secret');
      expect(within(secret).getByTestId('users-secret-value')).toHaveTextContent('token=abc123');
      expect(secret).toHaveAccessibleName('Invitation link for zed@example.test');
      expect(screen.queryByTestId('invite-dialog')).not.toBeInTheDocument();
      const post = api.calls.find((call) => call.method === 'POST');
      expect(post?.body).toEqual({ email: 'zed@example.test', role: 'admin' });
      expect(post?.headers.get('x-csrf-token')).toBe('csrf-admin');
      expect(
        api.calls.filter((call) => call.path === '/api/admin/invitations' && call.method === 'GET')
          .length,
      ).toBe(2);

      expect(screen.getByTestId('users-secret-done')).toBeDisabled();
      await user.click(screen.getByTestId('users-secret-ack'));
      await user.click(screen.getByTestId('users-secret-done'));
      expect(screen.queryByTestId('users-secret')).not.toBeInTheDocument();
      expect(screen.queryByText(/token=abc123/)).not.toBeInTheDocument();
    });

    it('shows a field problem from the server next to the field', async () => {
      const user = userEvent.setup();
      adminApi({
        'POST /api/admin/invitations': {
          status: 422,
          body: {
            error: {
              code: 'validation',
              message: 'Check the highlighted fields.',
              fields: [{ field: 'email', message: 'That email already has an account' }],
            },
          },
        },
      });
      await openPage();
      await user.click(screen.getByTestId('users-invite'));
      await user.type(screen.getByTestId('invite-email-input'), 'mia@example.test');
      await user.click(screen.getByTestId('invite-dialog-confirm'));
      expect(await screen.findByTestId('invite-email-error')).toHaveTextContent(
        'That email already has an account',
      );
      expect(screen.getByTestId('invite-dialog')).toHaveAttribute('open');
    });

    it('shows other failures in the dialog and keeps it open', async () => {
      const user = userEvent.setup();
      adminApi({ 'POST /api/admin/invitations': { status: 500, body: 'x' } });
      await openPage();
      await user.click(screen.getByTestId('users-invite'));
      await user.type(screen.getByTestId('invite-email-input'), 'ok@example.test');
      await user.click(screen.getByTestId('invite-dialog-confirm'));
      expect(await screen.findByTestId('invite-error')).toHaveTextContent(
        'Something went wrong. Nothing was changed.',
      );
    });

    it('has no accessibility violations while open', async () => {
      const user = userEvent.setup();
      adminApi();
      const { container } = await openPage();
      await user.click(screen.getByTestId('users-invite'));
      expect(await axe(container)).toHaveNoViolations();
    });
  });

  describe('keyboard operation', () => {
    it('opens with Enter, tabs through the fields, closes on Escape and returns focus', async () => {
      const user = userEvent.setup();
      adminApi();
      await openPage();
      const button = screen.getByTestId('users-invite');
      button.focus();
      await user.keyboard('{Enter}');
      const dialog = screen.getByTestId('invite-dialog');
      expect(dialog).toHaveAttribute('open');
      screen.getByTestId('invite-email-input').focus();
      await user.tab();
      expect(screen.getByTestId('invite-role-input')).toHaveFocus();
      await user.tab();
      expect(screen.getByTestId('invite-dialog-confirm')).toHaveFocus();
      await user.tab();
      expect(screen.getByTestId('invite-dialog-cancel')).toHaveFocus();
      dialog.dispatchEvent(new Event('cancel', { cancelable: true }));
      await waitFor(() => {
        expect(screen.queryByTestId('invite-dialog')).not.toBeInTheDocument();
      });
      await waitFor(() => {
        expect(button).toHaveFocus();
      });
    });
  });

  describe('changing a role', () => {
    it('sends the new role and refreshes the list', async () => {
      const user = userEvent.setup();
      const api = adminApi({ 'PUT /api/admin/users/{id}/role': { status: 204 } });
      api.on(`PUT /api/admin/users/${MIA.id}/role`, { status: 204 });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Change role for Mia Member' }));
      const dialog = screen.getByTestId('role-dialog');
      expect(
        within(dialog).getByRole('heading', { name: 'Change role for Mia Member' }),
      ).toBeInTheDocument();
      expect(screen.getByTestId('role-select-input')).toHaveValue('admin');
      await user.click(screen.getByTestId('role-dialog-confirm'));
      expect(await screen.findByText('Role changed')).toBeInTheDocument();
      expect(api.calls.find((call) => call.method === 'PUT')?.body).toEqual({ role: 'admin' });
      expect(
        api.calls.filter((call) => call.path === '/api/admin/users' && call.method === 'GET')
          .length,
      ).toBe(2);
      expect(screen.queryByTestId('role-dialog')).not.toBeInTheDocument();
    });

    it('shows the server answer when the change is refused', async () => {
      const user = userEvent.setup();
      adminApi({ [`PUT /api/admin/users/${BO.id}/role`]: LAST_ADMIN_ERROR });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Change role for Bo Builder' }));
      await user.click(screen.getByTestId('role-dialog-confirm'));
      expect(await screen.findByTestId('role-error')).toHaveTextContent(
        'At least one active admin is required',
      );
      expect(screen.getByTestId('role-dialog')).toHaveAttribute('open');
    });
  });

  describe('issuing a reset link', () => {
    it('explains the effect, optionally resets MFA, and shows the link once', async () => {
      const user = userEvent.setup();
      const api = adminApi({
        [`POST /api/admin/users/${MIA.id}/reset-link`]: {
          body: {
            link: 'https://tandem.example.test/reset-password?token=zzz',
            expires_at: '2026-03-03T00:00:00Z',
            user_display_name: 'Mia Member',
          },
        },
      });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Issue a reset link for Mia Member' }));
      expect(screen.getByTestId('reset-dialog')).toHaveTextContent('signs the user out everywhere');
      await user.click(screen.getByTestId('reset-also-mfa'));
      await user.click(screen.getByTestId('reset-dialog-confirm'));
      const secret = await screen.findByTestId('users-secret');
      expect(secret).toHaveAccessibleName('Reset link for Mia Member');
      expect(within(secret).getByTestId('users-secret-value')).toHaveTextContent(
        'reset-password?token=zzz',
      );
      expect(api.calls.find((call) => call.method === 'POST')?.body).toEqual({ reset_mfa: true });
      expect(screen.getByTestId('users-secret-expiry')).toHaveTextContent('stops working on');
      await user.click(screen.getByTestId('users-secret-ack'));
      await user.click(screen.getByTestId('users-secret-done'));
      expect(screen.queryByText(/token=zzz/)).not.toBeInTheDocument();
    });

    it('does not reset MFA unless asked', async () => {
      const user = userEvent.setup();
      const api = adminApi({
        [`POST /api/admin/users/${MIA.id}/reset-link`]: {
          body: {
            link: 'https://x.test/reset-password?token=q',
            expires_at: '2026-03-03T00:00:00Z',
            user_display_name: 'Mia Member',
          },
        },
      });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Issue a reset link for Mia Member' }));
      await user.click(screen.getByTestId('reset-dialog-confirm'));
      await screen.findByTestId('users-secret');
      expect(api.calls.find((call) => call.method === 'POST')?.body).toEqual({ reset_mfa: false });
    });

    it('shows a failure inside the dialog', async () => {
      const user = userEvent.setup();
      adminApi({ [`POST /api/admin/users/${MIA.id}/reset-link`]: { status: 500, body: 'x' } });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Issue a reset link for Mia Member' }));
      await user.click(screen.getByTestId('reset-dialog-confirm'));
      expect(await screen.findByTestId('reset-error')).toBeInTheDocument();
    });
  });

  describe('deactivating and reactivating', () => {
    it('needs the email typed, then deactivates', async () => {
      const user = userEvent.setup();
      const api = adminApi({ [`POST /api/admin/users/${MIA.id}/deactivate`]: { status: 204 } });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Deactivate Mia Member' }));
      const confirm = screen.getByTestId('action-dialog-confirm');
      expect(confirm).toBeDisabled();
      await user.type(screen.getByTestId('action-dialog-typed'), 'mia@example.test');
      await user.click(confirm);
      expect(await screen.findByText('User deactivated')).toBeInTheDocument();
      expect(
        api.calls.some((call) => call.method === 'POST' && call.path.endsWith('/deactivate')),
      ).toBe(true);
    });

    it('shows the last-admin message from the server and stays open', async () => {
      const user = userEvent.setup();
      adminApi({ [`POST /api/admin/users/${BO.id}/deactivate`]: LAST_ADMIN_ERROR });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Deactivate Bo Builder' }));
      await user.type(screen.getByTestId('action-dialog-typed'), 'bo@example.test');
      await user.click(screen.getByTestId('action-dialog-confirm'));
      expect(await screen.findByTestId('action-error')).toHaveTextContent(
        'At least one active admin is required',
      );
      expect(screen.getByTestId('action-dialog')).toHaveAttribute('open');
    });

    it('can be cancelled without calling the server', async () => {
      const user = userEvent.setup();
      const api = adminApi();
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Deactivate Mia Member' }));
      await user.click(screen.getByTestId('action-dialog-cancel'));
      expect(screen.queryByTestId('action-dialog')).not.toBeInTheDocument();
      expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
    });

    it('reactivates a deactivated user after one confirmation', async () => {
      const user = userEvent.setup();
      const api = adminApi({ [`POST /api/admin/users/${DEE.id}/reactivate`]: { status: 204 } });
      await openPage();
      await user.click(screen.getByRole('button', { name: 'Reactivate Dee Dormant' }));
      expect(screen.getByTestId('action-dialog')).toHaveTextContent('existing password');
      await user.click(screen.getByTestId('action-dialog-confirm'));
      expect(await screen.findByText('User reactivated')).toBeInTheDocument();
      expect(api.calls.some((call) => call.path === `/api/admin/users/${DEE.id}/reactivate`)).toBe(
        true,
      );
    });
  });

  describe('revoking an invitation', () => {
    it('deletes the invitation after confirmation', async () => {
      const user = userEvent.setup();
      const api = adminApi({ [`DELETE /api/admin/invitations/${PENDING.id}`]: { status: 204 } });
      await openPage();
      await user.click(
        screen.getByRole('button', { name: 'Revoke the invitation for new@example.test' }),
      );
      expect(screen.getByTestId('action-dialog')).toHaveTextContent('stops working straight away');
      await user.click(screen.getByTestId('action-dialog-confirm'));
      expect(await screen.findByText('Invitation revoked')).toBeInTheDocument();
      expect(api.calls.some((call) => call.method === 'DELETE')).toBe(true);
    });
  });
});
