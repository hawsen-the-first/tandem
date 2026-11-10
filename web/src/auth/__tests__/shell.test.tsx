import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { ME_ADMIN, ME_MEMBER, UNAUTHENTICATED, mockApi } from '../../test/mockApi';
import { renderApp } from '../../test/render';

describe('route guards', () => {
  it('sends a visitor who is not signed in to sign-in', async () => {
    mockApi({ 'GET /api/auth/me': UNAUTHENTICATED });
    renderApp('/chats');
    expect(await screen.findByRole('heading', { name: 'SignInPage' })).toBeInTheDocument();
  });

  it('shows the not-found page for unknown paths', async () => {
    mockApi({ 'GET /api/auth/me': UNAUTHENTICATED });
    renderApp('/definitely/not/here');
    expect(await screen.findByTestId('not-found-page')).toBeInTheDocument();
  });

  it('shows admin pages to admins', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    renderApp('/admin/users');
    expect(await screen.findByRole('heading', { name: 'UsersPage' })).toBeInTheDocument();
  });

  it('shows "not found" for admin pages to members (BR-9.2)', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_MEMBER } });
    renderApp('/admin/audit');
    expect(await screen.findByTestId('not-found-page')).toBeInTheDocument();
  });

  it('redirects an enrolment-only sign-in to MFA enrolment', async () => {
    mockApi({ 'GET /api/auth/me': { body: { ...ME_ADMIN, scope: 'mfa_enrolment_only' } } });
    renderApp('/chats');
    expect(await screen.findByRole('heading', { name: 'MfaEnrolmentPage' })).toBeInTheDocument();
  });

  it('lets an enrolment-only sign-in reach the enrolment page itself', async () => {
    mockApi({ 'GET /api/auth/me': { body: { ...ME_ADMIN, scope: 'mfa_enrolment_only' } } });
    renderApp('/mfa/enrol');
    expect(await screen.findByRole('heading', { name: 'MfaEnrolmentPage' })).toBeInTheDocument();
  });

  it('shows a retry when the session cannot be read at all', async () => {
    mockApi();
    renderApp('/');
    expect(await screen.findByTestId('session-error')).toBeInTheDocument();
  });
});

describe('application shell', () => {
  it('shows admin navigation to admins only', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    renderApp('/');
    const nav = await screen.findByRole('navigation', { name: 'Main navigation' });
    expect(within(nav).getByRole('link', { name: 'Users' })).toBeInTheDocument();
    expect(within(nav).getByRole('link', { name: 'Audit log' })).toBeInTheDocument();
  });

  it('hides admin navigation from members', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_MEMBER } });
    renderApp('/');
    const nav = await screen.findByRole('navigation', { name: 'Main navigation' });
    expect(within(nav).queryByRole('link', { name: 'Users' })).not.toBeInTheDocument();
    expect(within(nav).getByRole('link', { name: 'Chats' })).toBeInTheDocument();
  });

  it('has a skip link, a labelled landmark structure and no accessibility violations', async () => {
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    const { container } = renderApp('/');
    await screen.findByRole('navigation', { name: 'Main navigation' });
    expect(screen.getByRole('link', { name: 'Skip to content' })).toHaveAttribute('href', '#main');
    expect(screen.getByRole('main')).toBeInTheDocument();
    expect(screen.getByRole('banner')).toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('toggles the narrow-screen menu with aria-expanded', async () => {
    const user = userEvent.setup();
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    renderApp('/');
    const button = await screen.findByTestId('menu-button');
    expect(button).toHaveAttribute('aria-expanded', 'false');
    await user.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'true');
    await user.click(button);
    expect(button).toHaveAttribute('aria-expanded', 'false');
  });

  it('remembers the theme choice on the root element', async () => {
    const user = userEvent.setup();
    mockApi({ 'GET /api/auth/me': { body: ME_ADMIN } });
    renderApp('/');
    const toggle = await screen.findByTestId('theme-toggle');
    await user.selectOptions(toggle, 'dark');
    expect(document.documentElement).toHaveAttribute('data-theme', 'dark');
    expect(window.localStorage.getItem('tandem.theme')).toBe('dark');
    await user.selectOptions(toggle, 'system');
    expect(document.documentElement).not.toHaveAttribute('data-theme');
    expect(window.localStorage.getItem('tandem.theme')).toBeNull();
  });

  it('signs out through the API and returns to sign-in', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': { body: ME_ADMIN },
      'POST /api/auth/sign-out': { status: 204 },
    });
    renderApp('/');
    await user.click(await screen.findByText('Ada Admin'));
    await user.click(screen.getByTestId('sign-out'));
    await waitFor(() => {
      expect(
        api.calls.some((call) => call.method === 'POST' && call.path === '/api/auth/sign-out'),
      ).toBe(true);
    });
    expect(api.calls.find((call) => call.method === 'POST')?.headers.get('x-csrf-token')).toBe(
      'csrf-admin',
    );
  });
});
