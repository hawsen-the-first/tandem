import { ResponsiveTable } from '../../../components/ResponsiveTable';
import type { Column } from '../../../components/ResponsiveTable';
import { cx } from '../../../styles/cx';
import styles from '../admin.module.css';
import { FormattedTime, roleLabel, ta } from '../shared';
import type { AdminUser, Invitation, Row } from './types';
import { activeAdminCount, rowKey } from './types';

interface UserTableProps {
  rows: Row[];
  users: AdminUser[];
  /** The signed-in admin; actions on their own account are not offered. */
  currentUserId: string;
  onChangeRole: (user: AdminUser) => void;
  onResetLink: (user: AdminUser) => void;
  onDeactivate: (user: AdminUser) => void;
  onReactivate: (user: AdminUser) => void;
  onRevoke: (invitation: Invitation) => void;
}

function statusText(row: Row): string {
  if (row.kind === 'invitation') {
    return ta('statusPending');
  }
  return row.user.status === 'active' ? ta('statusActive') : ta('statusDeactivated');
}

export function UserTable({
  rows,
  users,
  currentUserId,
  onChangeRole,
  onResetLink,
  onDeactivate,
  onReactivate,
  onRevoke,
}: UserTableProps) {
  const adminCount = activeAdminCount(users);

  const actionsFor = (row: Row) => {
    if (row.kind === 'invitation') {
      return (
        <div className={styles.actions}>
          <button
            type="button"
            className={cx(styles.actionButton, styles.actionButtonDanger)}
            aria-label={ta('actionRevokeFor', { email: row.invitation.email })}
            data-testid="users-revoke"
            onClick={() => {
              onRevoke(row.invitation);
            }}
          >
            {ta('actionRevoke')}
          </button>
        </div>
      );
    }
    const { user } = row;
    const isSelf = user.id === currentUserId;
    const isLastAdmin = user.role === 'admin' && user.status === 'active' && adminCount <= 1;
    const reason = isSelf ? ta('reasonSelf') : isLastAdmin ? ta('reasonLastAdmin') : null;
    const reasonId = `reason-${user.id}`;
    const describedBy = reason === null ? undefined : reasonId;
    const deactivated = user.status === 'deactivated';
    return (
      <div className={styles.actions}>
        <button
          type="button"
          className={styles.actionButton}
          disabled={isSelf || isLastAdmin || deactivated}
          aria-describedby={describedBy}
          aria-label={ta('actionChangeRoleFor', { name: user.display_name })}
          data-testid="users-change-role"
          onClick={() => {
            onChangeRole(user);
          }}
        >
          {ta('actionChangeRole')}
        </button>
        <button
          type="button"
          className={styles.actionButton}
          disabled={isSelf || deactivated}
          aria-describedby={isSelf ? reasonId : undefined}
          aria-label={ta('actionResetLinkFor', { name: user.display_name })}
          data-testid="users-reset-link"
          onClick={() => {
            onResetLink(user);
          }}
        >
          {ta('actionResetLink')}
        </button>
        {deactivated ? (
          <button
            type="button"
            className={styles.actionButton}
            aria-label={ta('actionReactivateFor', { name: user.display_name })}
            data-testid="users-reactivate"
            onClick={() => {
              onReactivate(user);
            }}
          >
            {ta('actionReactivate')}
          </button>
        ) : (
          <button
            type="button"
            className={cx(styles.actionButton, styles.actionButtonDanger)}
            disabled={isSelf || isLastAdmin}
            aria-describedby={describedBy}
            aria-label={ta('actionDeactivateFor', { name: user.display_name })}
            data-testid="users-deactivate"
            onClick={() => {
              onDeactivate(user);
            }}
          >
            {ta('actionDeactivate')}
          </button>
        )}
        {reason === null ? null : (
          <p id={reasonId} className={styles.reason} data-testid="users-reason">
            {reason}
          </p>
        )}
      </div>
    );
  };

  const columns: Column<Row>[] = [
    {
      key: 'name',
      header: ta('colName'),
      render: (row) =>
        row.kind === 'user' ? (
          <>
            {row.user.display_name}
            {row.user.id === currentUserId ? ` ${ta('youMarker')}` : ''}
          </>
        ) : (
          ta('invitedPlaceholder')
        ),
    },
    {
      key: 'email',
      header: ta('colEmail'),
      render: (row) => (row.kind === 'user' ? row.user.email : row.invitation.email),
    },
    {
      key: 'role',
      header: ta('colRole'),
      render: (row) => roleLabel(row.kind === 'user' ? row.user.role : row.invitation.role),
    },
    {
      key: 'status',
      header: ta('colStatus'),
      render: (row) => (
        <span className={styles.status} data-testid="users-status">
          {statusText(row)}
        </span>
      ),
    },
    {
      key: 'lastSignIn',
      header: ta('colLastSignIn'),
      render: (row) => {
        if (row.kind === 'invitation') {
          return '—';
        }
        return row.user.last_sign_in_at == null ? (
          ta('neverSignedIn')
        ) : (
          <FormattedTime iso={row.user.last_sign_in_at} />
        );
      },
    },
    { key: 'actions', header: ta('colActions'), render: actionsFor },
  ];

  return (
    <ResponsiveTable
      caption={ta('usersTableCaption')}
      columns={columns}
      rows={rows}
      rowKey={rowKey}
      emptyState={ta('usersEmpty')}
      testId="users-table"
    />
  );
}
