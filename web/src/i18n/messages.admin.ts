// Text for the administration pages (users, settings, audit log). Wording for failures follows
// business-rules.md (Error handling); no message echoes a secret.

export const adminMessages = {
  usersTitle: 'Users',
  usersIntro:
    'People who can sign in to this Tandem instance, and invitations still waiting to be accepted.',
  usersTableCaption: 'Users and pending invitations',
  usersEmpty: 'No users yet',
  usersLoadError: 'Could not load the users. Try again.',
  usersInvite: 'Invite someone',

  colName: 'Name',
  colEmail: 'Email',
  colRole: 'Role',
  colStatus: 'Status',
  colLastSignIn: 'Last sign-in',
  colActions: 'Actions',

  roleAdmin: 'Admin',
  roleMember: 'Member',
  statusActive: 'Active',
  statusDeactivated: 'Deactivated',
  statusPending: 'Pending invitation',
  neverSignedIn: 'Never',
  invitedPlaceholder: 'Not accepted yet',
  youMarker: '(you)',

  actionChangeRole: 'Change role',
  actionChangeRoleFor: 'Change role for {name}',
  actionResetLink: 'Reset link',
  actionResetLinkFor: 'Issue a reset link for {name}',
  actionDeactivate: 'Deactivate',
  actionDeactivateFor: 'Deactivate {name}',
  actionReactivate: 'Reactivate',
  actionReactivateFor: 'Reactivate {name}',
  actionRevoke: 'Revoke',
  actionRevokeFor: 'Revoke the invitation for {email}',
  reasonSelf: 'Not available for your own account',
  reasonLastAdmin: 'At least one active admin is required',

  inviteTitle: 'Invite someone',
  inviteEmail: 'Email address',
  inviteRole: 'Role',
  inviteConfirm: 'Create invitation',
  inviteHint:
    'Nothing is emailed. You will get a link to send them yourself; it is shown once and works for 7 days.',
  inviteCreated: 'Invitation created',
  inviteLinkLabel: 'Invitation link for {email}',

  roleDialogTitle: 'Change role for {name}',
  roleDialogBody:
    'Admins can manage users, settings and the audit log. The change applies immediately.',
  roleDialogConfirm: 'Change role',
  roleChanged: 'Role changed',

  resetTitle: 'Issue a reset link for {name}',
  resetBody:
    'This signs the user out everywhere. They choose a new password from the link, which is shown once and works for 24 hours.',
  resetAlsoMfa: 'Also reset their two-step sign-in',
  resetMfaHint: 'Use this if they lost their device and recovery codes.',
  resetConfirm: 'Issue reset link',
  resetLinkLabel: 'Reset link for {name}',
  resetExpires: 'This link stops working on',

  deactivateTitle: 'Deactivate {name}?',
  deactivateBody:
    'They will be signed out everywhere and will not be able to sign in until reactivated. Their history is kept.',
  deactivateConfirm: 'Deactivate',
  deactivated: 'User deactivated',
  reactivateTitle: 'Reactivate {name}?',
  reactivateBody: 'They can sign in again with their existing password and two-step sign-in.',
  reactivateConfirm: 'Reactivate',
  reactivated: 'User reactivated',
  revokeTitle: 'Revoke the invitation for {email}?',
  revokeBody: 'The link stops working straight away.',
  revokeConfirm: 'Revoke invitation',
  revoked: 'Invitation revoked',

  settingsTitle: 'Sign-in settings',
  settingsIntro:
    'These apply to new sign-ins; people who are already signed in keep the limits they started with.',
  settingsLoadError: 'Could not load the settings. Try again.',
  settingsIdle: 'Sign out after being idle for (minutes)',
  settingsIdleHint:
    'Default 12 hours (720 minutes). Allowed: 15 minutes to 30 days (43,200 minutes).',
  settingsAbsolute: 'Sign out after this long in total (hours)',
  settingsAbsoluteHint: 'Default 14 days (336 hours). Allowed: 1 hour to 90 days (2,160 hours).',
  settingsOnline: 'Also check new passwords against the online breach service',
  settingsOnlineHint:
    'Passwords are always checked against the bundled list. The online check sends only a short hash prefix.',
  settingsSave: 'Save settings',
  settingsSaved: 'Settings saved. They apply to new sign-ins.',
  settingsIdleRange: 'Enter a whole number of minutes from 15 to 43,200',
  settingsAbsoluteRange: 'Enter a whole number of hours from 1 to 2,160',
  settingsIdleExceeds: 'The idle time cannot be longer than the total sign-in time',

  auditTitle: 'Audit log',
  auditIntro:
    'Entries are append-only: they cannot be edited or deleted here or anywhere in the application.',
  auditTableCaption: 'Audit entries, newest first',
  auditEmpty: 'No entries match these filters',
  auditLoadError: 'Could not load the audit log. Try again.',
  auditFilters: 'Filters',
  auditFilterUser: 'User',
  auditFilterAnyone: 'Anyone',
  auditFilterType: 'Event type',
  auditFilterAnyType: 'Any event',
  auditFilterFrom: 'From date',
  auditFilterTo: 'To date',
  auditApply: 'Apply filters',
  auditClear: 'Clear filters',
  auditLoadMore: 'Load more',
  auditLoadingMore: 'Loading…',
  auditColTime: 'Time',
  auditColActor: 'Actor',
  auditColEvent: 'Event',
  auditColObject: 'Object',
  auditColOutcome: 'Outcome',
  auditNoActor: 'System or not signed in',
  auditDateOrder: 'The start date must not be after the end date',
} as const;

export type AdminMessageKey = keyof typeof adminMessages;
