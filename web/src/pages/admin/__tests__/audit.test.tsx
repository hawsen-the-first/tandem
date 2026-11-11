import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { describe, expect, it } from 'vitest';
import { renderApp } from '../../../test/render';
import { auditQuery } from '../AuditLogPage';
import { AUDIT_EVENT_TYPES } from '../audit/eventTypes';
import { ADA, MIA, adminApi } from './fixtures';

function entry(index: number, overrides: Record<string, unknown> = {}) {
  return {
    id: `019c0000-0000-7000-8000-${String(index).padStart(12, '0')}`,
    occurred_at: `2026-03-02T09:${String(60 - index).padStart(2, '0')}:00Z`,
    actor_id: ADA.id,
    actor_label: 'Ada Admin',
    event_type: 'auth.sign_in.success',
    object_type: 'user',
    object_id: ADA.id,
    outcome: 'success',
    details: {},
    request_id: null,
    source: null,
    ...overrides,
  };
}

const PAGE_ONE = {
  entries: [entry(1), entry(2, { event_type: 'user.invited', actor_label: 'Ada Admin' })],
  next_after_time: '2026-03-02T09:58:00Z',
  next_after_id: 'cursor-id-2',
};
const PAGE_TWO = {
  entries: [
    entry(3, { event_type: 'auth.setup', actor_id: null, actor_label: 'x', object_type: null }),
  ],
  next_after_time: null,
  next_after_id: null,
};

function auditCalls(api: ReturnType<typeof adminApi>) {
  return api.calls.filter((call) => call.path === '/api/admin/audit');
}

async function openPage(handler: Parameters<typeof adminApi>[0] = {}) {
  const api = adminApi({ 'GET /api/admin/audit': { body: PAGE_ONE }, ...handler }, [ADA, MIA]);
  const view = renderApp('/admin/audit');
  await screen.findByTestId('audit-table');
  return { api, ...view };
}

describe('audit event types', () => {
  it('lists the 20 catalogue names, each unique and dotted', () => {
    expect(AUDIT_EVENT_TYPES).toHaveLength(20);
    expect(new Set(AUDIT_EVENT_TYPES).size).toBe(20);
    for (const type of AUDIT_EVENT_TYPES) {
      expect(type).toMatch(/^[a-z_]+(\.[a-z_]+)+$/);
    }
  });
});

describe('auditQuery', () => {
  it('always asks for a page size and only includes what is set', () => {
    expect(auditQuery({ actorId: '', eventType: '', fromDate: '', toDate: '' }, null)).toEqual({
      limit: 50,
    });
    const query = auditQuery(
      { actorId: 'u1', eventType: 'user.invited', fromDate: '2026-03-01', toDate: '2026-03-02' },
      { time: 't', id: 'i' },
    );
    expect(query).toMatchObject({
      actor_id: 'u1',
      event_type: 'user.invited',
      after_time: 't',
      after_id: 'i',
      limit: 50,
    });
    expect(new Date(query.from ?? '').getTime()).toBeLessThan(new Date(query.to ?? '').getTime());
  });
});

describe('AuditLogPage', () => {
  it('shows entries newest first with the append-only note and no edit or delete controls', async () => {
    const { container } = (await openPage()) as { container: HTMLElement };
    expect(screen.getByTestId('audit-note')).toHaveTextContent('append-only');
    const rows = screen.getAllByTestId('audit-table-row');
    expect(rows).toHaveLength(2);
    expect(within(rows[0] as HTMLElement).getByText('auth.sign_in.success')).toBeInTheDocument();
    expect(
      within(rows[0] as HTMLElement).getByText((_, element) => element?.tagName === 'TIME'),
    ).toHaveAttribute('datetime', '2026-03-02T09:59:00Z');
    expect(screen.queryByRole('button', { name: /delete|edit|remove/i })).not.toBeInTheDocument();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('loads the first page with a page size and no filters', async () => {
    const { api } = await openPage();
    expect(auditCalls(api)).toHaveLength(1);
    expect(auditCalls(api)[0]?.search).toBe('?limit=50');
  });

  it('loads more using the cursor from the previous page and appends', async () => {
    const user = userEvent.setup();
    const { api } = await openPage();
    api.on('GET /api/admin/audit', { body: PAGE_TWO });
    await user.click(screen.getByTestId('audit-load-more'));
    await waitFor(() => {
      expect(screen.getAllByTestId('audit-table-row')).toHaveLength(3);
    });
    const query = new URLSearchParams(auditCalls(api)[1]?.search);
    expect(query.get('after_time')).toBe('2026-03-02T09:58:00Z');
    expect(query.get('after_id')).toBe('cursor-id-2');
    expect(screen.queryByTestId('audit-load-more')).not.toBeInTheDocument();
  });

  it('offers users and the 20 event types as filter choices', async () => {
    await openPage();
    const users = within(screen.getByTestId('audit-user-input')).getAllByRole('option');
    expect(users.map((option) => option.textContent)).toEqual([
      'Anyone',
      'Ada Admin',
      'Mia Member',
    ]);
    expect(within(screen.getByTestId('audit-type-input')).getAllByRole('option')).toHaveLength(21);
  });

  it('applies filters and starts again from the newest page', async () => {
    const user = userEvent.setup();
    const { api } = await openPage();
    api.on('GET /api/admin/audit', { body: PAGE_TWO });
    await user.selectOptions(screen.getByTestId('audit-user-input'), MIA.id);
    await user.selectOptions(screen.getByTestId('audit-type-input'), 'auth.setup');
    await user.type(screen.getByTestId('audit-from-input'), '2026-03-01');
    await user.type(screen.getByTestId('audit-to-input'), '2026-03-02');
    await user.click(screen.getByTestId('audit-apply'));
    await waitFor(() => {
      expect(screen.getAllByTestId('audit-table-row')).toHaveLength(1);
    });
    const query = new URLSearchParams(auditCalls(api).at(-1)?.search);
    expect(query.get('actor_id')).toBe(MIA.id);
    expect(query.get('event_type')).toBe('auth.setup');
    expect(query.get('from')).not.toBeNull();
    expect(query.get('to')).not.toBeNull();
    expect(query.has('after_time')).toBe(false);
  });

  it('does not send a request for a start date after the end date', async () => {
    const user = userEvent.setup();
    const { api } = await openPage();
    await user.type(screen.getByTestId('audit-from-input'), '2026-03-05');
    await user.type(screen.getByTestId('audit-to-input'), '2026-03-01');
    await user.click(screen.getByTestId('audit-apply'));
    expect(screen.getByTestId('audit-to-error')).toHaveTextContent('start date must not be after');
    expect(auditCalls(api)).toHaveLength(1);
  });

  it('clears the filters', async () => {
    const user = userEvent.setup();
    const { api } = await openPage();
    await user.selectOptions(screen.getByTestId('audit-type-input'), 'auth.setup');
    await user.click(screen.getByTestId('audit-apply'));
    await waitFor(() => {
      expect(auditCalls(api).length).toBe(2);
    });
    await user.click(screen.getByTestId('audit-clear'));
    await waitFor(() => {
      expect(auditCalls(api).at(-1)?.search).toBe('?limit=50');
    });
    expect(screen.getByTestId('audit-type-input')).toHaveValue('');
  });

  it('shows an empty state when nothing matches', async () => {
    adminApi(
      {
        'GET /api/admin/audit': {
          body: { entries: [], next_after_time: null, next_after_id: null },
        },
      },
      [ADA],
    );
    renderApp('/admin/audit');
    expect(await screen.findByTestId('audit-table-empty')).toHaveTextContent('No entries match');
  });

  it('shows a loading state', async () => {
    adminApi(
      {
        'GET /api/admin/audit': async () => {
          await new Promise((resolve) => setTimeout(resolve, 30));
          return { body: PAGE_ONE };
        },
      },
      [ADA],
    );
    renderApp('/admin/audit');
    expect(await screen.findByTestId('audit-loading')).toBeInTheDocument();
    expect(await screen.findByTestId('audit-table')).toBeInTheDocument();
  });

  it('shows an error with a retry', async () => {
    const user = userEvent.setup();
    const api = adminApi({ 'GET /api/admin/audit': { status: 500, body: 'x' } }, [ADA]);
    const { container } = renderApp('/admin/audit');
    const error = await screen.findByTestId('audit-error');
    expect(await axe(container)).toHaveNoViolations();
    api.on('GET /api/admin/audit', { body: PAGE_ONE });
    await user.click(within(error).getByRole('button', { name: 'Try again' }));
    expect(await screen.findByTestId('audit-table')).toBeInTheDocument();
  });

  it('can be operated by keyboard: filter, apply, load more', async () => {
    const user = userEvent.setup();
    const { api } = await openPage();
    api.on('GET /api/admin/audit', { body: PAGE_TWO });
    screen.getByTestId('audit-type-input').focus();
    await user.keyboard('{ArrowDown}');
    await user.keyboard('{Enter}');
    await waitFor(() => {
      expect(auditCalls(api).length).toBeGreaterThanOrEqual(1);
    });
    screen.getByTestId('audit-load-more').focus();
    await user.keyboard('{Enter}');
    await waitFor(() => {
      expect(screen.getAllByTestId('audit-table-row').length).toBeGreaterThanOrEqual(2);
    });
  });
});
