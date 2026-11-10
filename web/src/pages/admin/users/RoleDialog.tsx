import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage } from '../../../api/messages';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { FormField } from '../../../components/FormField';
import { useToast } from '../../../components/Toast';
import { cx } from '../../../styles/cx';
import sharedStyles from '../../../styles/shared.module.css';
import styles from '../admin.module.css';
import { ta } from '../shared';
import type { AdminUser } from './types';

interface RoleDialogProps {
  user: AdminUser;
  onClose: () => void;
}

export function RoleDialog({ user, onClose }: RoleDialogProps) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [role, setRole] = useState(user.role === 'admin' ? 'member' : 'admin');
  const [failure, setFailure] = useState<string | null>(null);

  const change = useMutation({
    mutationFn: () =>
      call(
        client.PUT('/api/admin/users/{id}/role', {
          params: { path: { id: user.id } },
          body: { role },
        }),
      ),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ['admin', 'users'] });
      toast.show('success', ta('roleChanged'));
      onClose();
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  return (
    <ConfirmDialog
      open
      title={ta('roleDialogTitle', { name: user.display_name })}
      confirmLabel={ta('roleDialogConfirm')}
      busy={change.isPending}
      onConfirm={() => {
        setFailure(null);
        change.mutate();
      }}
      onCancel={onClose}
      testId="role-dialog"
    >
      <div className={styles.formGrid}>
        <p>{ta('roleDialogBody')}</p>
        <FormField label={ta('colRole')} testId="role-select">
          {(control) => (
            <select
              {...control}
              value={role}
              data-testid="role-select-input"
              onChange={(event) => {
                setRole(event.target.value);
              }}
            >
              <option value="member">{ta('roleMember')}</option>
              <option value="admin">{ta('roleAdmin')}</option>
            </select>
          )}
        </FormField>
        {failure === null ? null : (
          <p
            role="alert"
            className={cx(sharedStyles.banner, sharedStyles.bannerError)}
            data-testid="role-error"
          >
            {failure}
          </p>
        )}
      </div>
    </ConfirmDialog>
  );
}
