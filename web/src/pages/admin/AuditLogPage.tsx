import { useInfiniteQuery, useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../api/client';
import { errorMessage } from '../../api/messages';
import type { components, paths } from '../../api/schema';
import { FormField } from '../../components/FormField';
import { ResponsiveTable } from '../../components/ResponsiveTable';
import type { Column } from '../../components/ResponsiveTable';
import { t } from '../../i18n/messages';
import { cx } from '../../styles/cx';
import sharedStyles from '../../styles/shared.module.css';
import styles from './admin.module.css';
import { AUDIT_EVENT_TYPES } from './audit/eventTypes';
import { FormattedTime, ta } from './shared';

type Entry = components['schemas']['AuditEntryDto'];

const PAGE_SIZE = 50;

interface Filters {
  actorId: string;
  eventType: string;
  fromDate: string;
  toDate: string;
}

const NO_FILTERS: Filters = { actorId: '', eventType: '', fromDate: '', toDate: '' };

interface Cursor {
  time: string;
  id: string;
}

/** Local midnight at the start of `date` (yyyy-mm-dd) as an RFC 3339 instant. */
function startOfDay(date: string): string {
  return new Date(`${date}T00:00:00`).toISOString();
}

function endOfDay(date: string): string {
  return new Date(`${date}T23:59:59.999`).toISOString();
}

export function auditQuery(
  filters: Filters,
  cursor: Cursor | null,
): NonNullable<paths['/api/admin/audit']['get']['parameters']['query']> {
  return {
    ...(filters.actorId === '' ? {} : { actor_id: filters.actorId }),
    ...(filters.eventType === '' ? {} : { event_type: filters.eventType }),
    ...(filters.fromDate === '' ? {} : { from: startOfDay(filters.fromDate) }),
    ...(filters.toDate === '' ? {} : { to: endOfDay(filters.toDate) }),
    ...(cursor === null ? {} : { after_time: cursor.time, after_id: cursor.id }),
    limit: PAGE_SIZE,
  };
}

export function Component() {
  const [draft, setDraft] = useState<Filters>(NO_FILTERS);
  const [applied, setApplied] = useState<Filters>(NO_FILTERS);
  const [dateProblem, setDateProblem] = useState<string | undefined>(undefined);

  const users = useQuery({
    queryKey: ['admin', 'users'],
    queryFn: () => call(client.GET('/api/admin/users')),
  });

  // The filters are part of the key, so changing them starts again from the newest page.
  const audit = useInfiniteQuery({
    queryKey: ['admin', 'audit', applied],
    initialPageParam: null as Cursor | null,
    queryFn: ({ pageParam }) =>
      call(client.GET('/api/admin/audit', { params: { query: auditQuery(applied, pageParam) } })),
    getNextPageParam: (last): Cursor | null =>
      last.next_after_time != null && last.next_after_id != null
        ? { time: last.next_after_time, id: last.next_after_id }
        : null,
  });

  const entries: Entry[] = audit.data?.pages.flatMap((page) => page.entries) ?? [];

  const columns: Column<Entry>[] = [
    {
      key: 'time',
      header: ta('auditColTime'),
      render: (entry) => <FormattedTime iso={entry.occurred_at} />,
    },
    {
      key: 'actor',
      header: ta('auditColActor'),
      render: (entry) => (entry.actor_id == null ? ta('auditNoActor') : entry.actor_label),
    },
    {
      key: 'event',
      header: ta('auditColEvent'),
      render: (entry) => <code>{entry.event_type}</code>,
    },
    {
      key: 'object',
      header: ta('auditColObject'),
      render: (entry) =>
        entry.object_type == null
          ? '—'
          : `${entry.object_type}${entry.object_id == null ? '' : ` ${entry.object_id}`}`,
    },
    { key: 'outcome', header: ta('auditColOutcome'), render: (entry) => entry.outcome },
  ];

  return (
    <>
      <h1>{ta('auditTitle')}</h1>
      <p className={sharedStyles.muted} data-testid="audit-note">
        {ta('auditIntro')}
      </p>

      <form
        className={styles.filters}
        aria-label={ta('auditFilters')}
        data-testid="audit-filters"
        onSubmit={(event) => {
          event.preventDefault();
          if (draft.fromDate !== '' && draft.toDate !== '' && draft.fromDate > draft.toDate) {
            setDateProblem(ta('auditDateOrder'));
            return;
          }
          setDateProblem(undefined);
          setApplied(draft);
        }}
      >
        <FormField label={ta('auditFilterUser')} testId="audit-user">
          {(control) => (
            <select
              {...control}
              value={draft.actorId}
              data-testid="audit-user-input"
              onChange={(event) => {
                setDraft({ ...draft, actorId: event.target.value });
              }}
            >
              <option value="">{ta('auditFilterAnyone')}</option>
              {(users.data ?? []).map((user) => (
                <option key={user.id} value={user.id}>
                  {user.display_name}
                </option>
              ))}
            </select>
          )}
        </FormField>
        <FormField label={ta('auditFilterType')} testId="audit-type">
          {(control) => (
            <select
              {...control}
              value={draft.eventType}
              data-testid="audit-type-input"
              onChange={(event) => {
                setDraft({ ...draft, eventType: event.target.value });
              }}
            >
              <option value="">{ta('auditFilterAnyType')}</option>
              {AUDIT_EVENT_TYPES.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          )}
        </FormField>
        <FormField label={ta('auditFilterFrom')} testId="audit-from">
          {(control) => (
            <input
              {...control}
              type="date"
              value={draft.fromDate}
              data-testid="audit-from-input"
              onChange={(event) => {
                setDraft({ ...draft, fromDate: event.target.value });
              }}
            />
          )}
        </FormField>
        <FormField label={ta('auditFilterTo')} error={dateProblem} testId="audit-to">
          {(control) => (
            <input
              {...control}
              type="date"
              value={draft.toDate}
              data-testid="audit-to-input"
              onChange={(event) => {
                setDraft({ ...draft, toDate: event.target.value });
              }}
            />
          )}
        </FormField>
        <div className={styles.filterActions}>
          <button type="submit" className={sharedStyles.button} data-testid="audit-apply">
            {ta('auditApply')}
          </button>
          <button
            type="button"
            className={cx(sharedStyles.button, sharedStyles.secondary)}
            data-testid="audit-clear"
            onClick={() => {
              setDraft(NO_FILTERS);
              setApplied(NO_FILTERS);
              setDateProblem(undefined);
            }}
          >
            {ta('auditClear')}
          </button>
        </div>
      </form>

      {audit.isPending ? (
        <p role="status" data-testid="audit-loading">
          {t('loading')}
        </p>
      ) : audit.isError ? (
        <div
          role="alert"
          className={cx(sharedStyles.banner, sharedStyles.bannerError)}
          data-testid="audit-error"
        >
          <p>{ta('auditLoadError')}</p>
          <p>{errorMessage(audit.error)}</p>
          <button
            type="button"
            className={sharedStyles.button}
            onClick={() => {
              void audit.refetch();
            }}
          >
            {t('retry')}
          </button>
        </div>
      ) : (
        <>
          <ResponsiveTable
            caption={ta('auditTableCaption')}
            columns={columns}
            rows={entries}
            rowKey={(entry) => entry.id}
            emptyState={ta('auditEmpty')}
            testId="audit-table"
          />
          {audit.hasNextPage ? (
            <p>
              <button
                type="button"
                className={cx(sharedStyles.button, sharedStyles.secondary)}
                disabled={audit.isFetchingNextPage}
                data-testid="audit-load-more"
                onClick={() => {
                  void audit.fetchNextPage();
                }}
              >
                {audit.isFetchingNextPage ? ta('auditLoadingMore') : ta('auditLoadMore')}
              </button>
            </p>
          ) : null}
        </>
      )}
    </>
  );
}
