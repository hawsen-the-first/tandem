import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage, fieldErrors } from '../../../api/messages';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { FormField } from '../../../components/FormField';
import { messages as shared } from '../../../i18n/messages';
import { cx } from '../../../styles/cx';
import sharedStyles from '../../../styles/shared.module.css';
import styles from '../admin.module.css';
import { ta } from '../shared';

const EMAIL_SHAPE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

interface InviteDialogProps {
  onClose: () => void;
  /** Called with the one-time link; the page shows it and clears it on dismissal. */
  onCreated: (email: string, link: string) => void;
}

export function InviteDialog({ onClose, onCreated }: InviteDialogProps) {
  const queryClient = useQueryClient();
  const [email, setEmail] = useState('');
  const [role, setRole] = useState('member');
  const [problems, setProblems] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);

  const invite = useMutation({
    mutationFn: (input: { email: string; role: string }) =>
      call(client.POST('/api/admin/invitations', { body: input })),
    onSuccess: async (created, input) => {
      await queryClient.invalidateQueries({ queryKey: ['admin', 'invitations'] });
      onCreated(input.email, created.link);
    },
    onError: (error) => {
      setProblems(fieldErrors(error));
      setFailure(errorMessage(error));
    },
  });

  const submit = () => {
    const trimmed = email.trim();
    if (trimmed.length === 0 || trimmed.length > 254 || !EMAIL_SHAPE.test(trimmed)) {
      setProblems({ email: shared.validationEmail });
      setFailure(null);
      return;
    }
    setProblems({});
    setFailure(null);
    invite.mutate({ email: trimmed, role });
  };

  return (
    <ConfirmDialog
      open
      title={ta('inviteTitle')}
      confirmLabel={ta('inviteConfirm')}
      busy={invite.isPending}
      onConfirm={submit}
      onCancel={onClose}
      testId="invite-dialog"
    >
      <div className={styles.formGrid}>
        <p>{ta('inviteHint')}</p>
        <FormField
          label={ta('inviteEmail')}
          error={problems['email']}
          required
          testId="invite-email"
        >
          {(control) => (
            <input
              {...control}
              type="email"
              autoComplete="off"
              value={email}
              data-testid="invite-email-input"
              onChange={(event) => {
                setEmail(event.target.value);
              }}
            />
          )}
        </FormField>
        <FormField label={ta('inviteRole')} error={problems['role']} testId="invite-role">
          {(control) => (
            <select
              {...control}
              value={role}
              data-testid="invite-role-input"
              onChange={(event) => {
                setRole(event.target.value);
              }}
            >
              <option value="member">{ta('roleMember')}</option>
              <option value="admin">{ta('roleAdmin')}</option>
            </select>
          )}
        </FormField>
        {failure !== null && problems['email'] === undefined ? (
          <p
            role="alert"
            className={cx(sharedStyles.banner, sharedStyles.bannerError)}
            data-testid="invite-error"
          >
            {failure}
          </p>
        ) : null}
      </div>
    </ConfirmDialog>
  );
}
