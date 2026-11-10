import type { components } from '../../../api/schema';

export type AdminUser = components['schemas']['AdminUserDto'];
export type Invitation = components['schemas']['InvitationDto'];

export type Row =
  { kind: 'user'; user: AdminUser } | { kind: 'invitation'; invitation: Invitation };

/** Active users and still-pending invitations as one list; settled invitations are not shown. */
export function mergeRows(users: AdminUser[], invitations: Invitation[]): Row[] {
  const userRows: Row[] = users.map((user) => ({ kind: 'user', user }));
  const inviteRows: Row[] = invitations
    .filter((invitation) => invitation.status === 'pending')
    .map((invitation) => ({ kind: 'invitation', invitation }));
  return [...userRows, ...inviteRows];
}

export function activeAdminCount(users: AdminUser[]): number {
  return users.filter((user) => user.role === 'admin' && user.status === 'active').length;
}

export function rowKey(row: Row): string {
  return row.kind === 'user' ? `user-${row.user.id}` : `invitation-${row.invitation.id}`;
}
