import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import { call, client } from '../../api/client';
import { errorMessage } from '../../api/messages';
import { CopyOnceSecret } from '../../components/CopyOnceSecret';
import { useToast } from '../../components/Toast';
import { t } from '../../i18n/messages';
import { useSession } from '../../auth/SessionProvider';
import { cx } from '../../styles/cx';
import sharedStyles from '../../styles/shared.module.css';
import styles from './admin.module.css';
import { FormattedTime, ta } from './shared';
import { ActionDialog } from './users/ActionDialog';
import type { ActionTarget } from './users/ActionDialog';
import { InviteDialog } from './users/InviteDialog';
import { ResetLinkDialog } from './users/ResetLinkDialog';
import { RoleDialog } from './users/RoleDialog';
import { UserTable } from './users/UserTable';
import { mergeRows } from './users/types';
import type { AdminUser } from './users/types';

type Dialog =
  | { kind: 'invite' }
  | { kind: 'role'; user: AdminUser }
  | { kind: 'reset'; user: AdminUser }
  | { kind: 'action'; target: ActionTarget };

interface OneTimeSecret {
  label: string;
  link: string;
  expiresAt?: string;
}

export function Component() {
  const { me } = useSession();
  const toast = useToast();
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const [secret, setSecret] = useState<OneTimeSecret | null>(null);
  const opener = useRef<HTMLElement | null>(null);
  const secretPanel = useRef<HTMLDivElement>(null);

  const users = useQuery({
    queryKey: ['admin', 'users'],
    queryFn: () => call(client.GET('/api/admin/users')),
  });
  const invitations = useQuery({
    queryKey: ['admin', 'invitations'],
    queryFn: () => call(client.GET('/api/admin/invitations')),
  });

  const open = useCallback((next: Dialog) => {
    opener.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    setDialog(next);
  }, []);

  // Closing returns focus to the control that opened the dialog.
  const close = useCallback(() => {
    setDialog(null);
    const target = opener.current;
    opener.current = null;
    window.setTimeout(() => {
      target?.focus();
    }, 0);
  }, []);

  useEffect(() => {
    if (secret !== null) {
      secretPanel.current?.focus();
    }
  }, [secret]);

  if (users.isPending || invitations.isPending) {
    return (
      <p role="status" className={sharedStyles.muted} data-testid="users-loading">
        {t('loading')}
      </p>
    );
  }
  if (users.isError || invitations.isError) {
    return (
      <div
        role="alert"
        className={cx(sharedStyles.banner, sharedStyles.bannerError)}
        data-testid="users-error"
      >
        <p>{ta('usersLoadError')}</p>
        <p>{errorMessage(users.error ?? invitations.error)}</p>
        <button
          type="button"
          className={sharedStyles.button}
          onClick={() => {
            void users.refetch();
            void invitations.refetch();
          }}
        >
          {t('retry')}
        </button>
      </div>
    );
  }

  const rows = mergeRows(users.data, invitations.data);

  return (
    <>
      <div className={styles.header}>
        <h1>{ta('usersTitle')}</h1>
        <button
          type="button"
          className={sharedStyles.button}
          data-testid="users-invite"
          onClick={() => {
            open({ kind: 'invite' });
          }}
        >
          {ta('usersInvite')}
        </button>
      </div>
      <p className={sharedStyles.muted}>{ta('usersIntro')}</p>

      {secret === null ? null : (
        <div
          ref={secretPanel}
          tabIndex={-1}
          className={styles.secretPanel}
          data-testid="users-secret-panel"
        >
          {secret.expiresAt === undefined ? null : (
            <p data-testid="users-secret-expiry">
              {ta('resetExpires')} <FormattedTime iso={secret.expiresAt} />.
            </p>
          )}
          <CopyOnceSecret
            label={secret.label}
            secret={[secret.link]}
            testId="users-secret"
            onDismiss={() => {
              setSecret(null);
            }}
          />
        </div>
      )}

      <UserTable
        rows={rows}
        users={users.data}
        currentUserId={me?.user.id ?? ''}
        onChangeRole={(user) => {
          open({ kind: 'role', user });
        }}
        onResetLink={(user) => {
          open({ kind: 'reset', user });
        }}
        onDeactivate={(user) => {
          open({ kind: 'action', target: { action: 'deactivate', user } });
        }}
        onReactivate={(user) => {
          open({ kind: 'action', target: { action: 'reactivate', user } });
        }}
        onRevoke={(invitation) => {
          open({ kind: 'action', target: { action: 'revoke', invitation } });
        }}
      />

      {dialog?.kind === 'invite' ? (
        <InviteDialog
          onClose={close}
          onCreated={(email, link) => {
            toast.show('success', ta('inviteCreated'));
            setSecret({ label: ta('inviteLinkLabel', { email }), link });
            close();
          }}
        />
      ) : null}
      {dialog?.kind === 'role' ? (
        <RoleDialog key={dialog.user.id} user={dialog.user} onClose={close} />
      ) : null}
      {dialog?.kind === 'reset' ? (
        <ResetLinkDialog
          key={dialog.user.id}
          user={dialog.user}
          onClose={close}
          onIssued={(user, link, expiresAt) => {
            setSecret({
              label: ta('resetLinkLabel', { name: user.display_name }),
              link,
              expiresAt,
            });
            close();
          }}
        />
      ) : null}
      {dialog?.kind === 'action' ? <ActionDialog target={dialog.target} onClose={close} /> : null}
    </>
  );
}
