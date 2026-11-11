import { screen } from '@testing-library/react';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { ME_ADMIN, ME_MEMBER, mockApi } from '../../test/mockApi';
import { renderApp } from '../../test/render';

const session = (n: number) => ({
  id: `s${String(n)}`,
  title: `Chat ${String(n)}`,
  created_at: '2026-01-01T00:00:00Z',
});

describe('HomePage', () => {
  it('shows recent chats (at most five) and a way to start a new one', async () => {
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [1, 2, 3, 4, 5, 6, 7].map(session) },
    });
    const { container } = renderApp('/');
    const list = await screen.findByTestId('home-recent-list');
    expect(list.querySelectorAll('li')).toHaveLength(5);
    expect(screen.getByRole('link', { name: 'Chat 1' })).toHaveAttribute('href', '/chats/s1');
    expect(screen.getByTestId('home-new-chat')).toHaveAttribute('href', '/chats');
    expect(screen.queryByTestId('home-recovery-reminder')).not.toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('says so when there are no chats', async () => {
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [] },
    });
    renderApp('/');
    expect(await screen.findByTestId('home-no-chats')).toBeInTheDocument();
  });

  it('reminds people whose recovery codes are running low, linking to the account page', async () => {
    mockApi({
      'GET /api/auth/me': {
        body: {
          ...ME_ADMIN,
          mfa: { enrolled: true, recovery_codes_remaining: 2, remind_regenerate: true },
        },
      },
      'GET /api/chat/sessions': { body: [] },
    });
    const { container } = renderApp('/');
    const reminder = await screen.findByTestId('home-recovery-reminder');
    expect(reminder).toHaveTextContent('recovery codes are running low');
    expect(screen.getByRole('link', { name: 'Go to your account' })).toHaveAttribute(
      'href',
      '/account',
    );
    expect(await axe(container)).toHaveNoViolations();
  });
});
