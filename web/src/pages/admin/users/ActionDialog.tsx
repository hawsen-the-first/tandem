import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage } from '../../../api/messages';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { useToast } from '../../../components/Toast';
import { cx } from '../../../styles/cx';
import sharedStyles from '../../../styles/shared.module.css';
import { ta } from '../shared';
import type { AdminUser, Invitation } from './types';

export type ActionTarget =
  | { action: 'deactivate'; user: AdminUser }
  | { action: 'reactivate'; user: AdminUser }
  | { action: 'revoke'; invitation: Invitation };

interface ActionDialogProps {
  target: ActionTarget;
  onClose: () => void;
}

/** Deactivate, reactivate or revoke: one explicit confirmation each, with the server's answer shown on failure. */
export function ActionDialog({ target, onClose }: ActionDialogProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [failure, setFailure] = useState<string | null>(null);

  const run = useMutation({
    mutationFn: async () => {
      switch (target.action) {
        case 'deactivate':
          await call(
            client.POST('/api/admin/users/{id}/deactivate', {
              params: { path: { id: target.user.id } },
            }),
          );
          return;
        case 'reactivate':
          await call(
            client.POST('/api/admin/users/{id}/reactivate', {
              params: { path: { id: target.user.id } },
            }),
          );
          return;
        case 'revoke':
          await call(
            client.DELETE('/api/admin/invitations/{id}', {
              params: { path: { id: target.invitation.id } },
            }),
          );
      }
    },
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['admin'] });
      toast.show(
        'success',
        target.action === 'deactivate'
          ? ta('deactivated')
          : target.action === 'reactivate'
            ? ta('reactivated')
            : ta('revoked'),
      );
      onClose();
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  const copy =
    target.action === 'deactivate'
      ? {
          title: ta('deactivateTitle', { name: target.user.display_name }),
          body: ta('deactivateBody'),
          confirm: ta('deactivateConfirm'),
        }
      : target.action === 'reactivate'
        ? {
            title: ta('reactivateTitle', { name: target.user.display_name }),
            body: ta('reactivateBody'),
            confirm: ta('reactivateConfirm'),
          }
        : {
            title: ta('revokeTitle', { email: target.invitation.email }),
            body: ta('revokeBody'),
            confirm: ta('revokeConfirm'),
          };

  return (
    <ConfirmDialog
      open
      title={copy.title}
      confirmLabel={copy.confirm}
      destructive={target.action !== 'reactivate'}
      {...(target.action === 'deactivate' ? { requireTyped: target.user.email } : {})}
      busy={run.isPending}
      onConfirm={() => {
        setFailure(null);
        run.mutate();
      }}
      onCancel={onClose}
      testId="action-dialog"
    >
      <p>{copy.body}</p>
      {failure === null ? null : (
        <p
          role="alert"
          className={cx(sharedStyles.banner, sharedStyles.bannerError)}
          data-testid="action-error"
        >
          {failure}
        </p>
      )}
    </ConfirmDialog>
  );
}
