import { useMutation } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../../api/client';
import { errorMessage } from '../../../api/messages';
import { ConfirmDialog } from '../../../components/ConfirmDialog';
import { cx } from '../../../styles/cx';
import sharedStyles from '../../../styles/shared.module.css';
import styles from '../admin.module.css';
import { ta } from '../shared';
import type { AdminUser } from './types';

interface ResetLinkDialogProps {
  user: AdminUser;
  onClose: () => void;
  /** Called with the one-time link and its expiry; the page owns the secret from here on. */
  onIssued: (user: AdminUser, link: string, expiresAt: string) => void;
}

export function ResetLinkDialog({ user, onClose, onIssued }: ResetLinkDialogProps) {
  const [alsoMfa, setAlsoMfa] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const issue = useMutation({
    mutationFn: () =>
      call(
        client.POST('/api/admin/users/{id}/reset-link', {
          params: { path: { id: user.id } },
          body: { reset_mfa: alsoMfa },
        }),
      ),
    onSuccess: (issued) => {
      onIssued(user, issued.link, issued.expires_at);
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  return (
    <ConfirmDialog
      open
      title={ta('resetTitle', { name: user.display_name })}
      confirmLabel={ta('resetConfirm')}
      busy={issue.isPending}
      onConfirm={() => {
        setFailure(null);
        issue.mutate();
      }}
      onCancel={onClose}
      testId="reset-dialog"
    >
      <div className={styles.formGrid}>
        <p>{ta('resetBody')}</p>
        <label className={styles.checkboxRow}>
          <input
            type="checkbox"
            checked={alsoMfa}
            data-testid="reset-also-mfa"
            onChange={(event) => {
              setAlsoMfa(event.target.checked);
            }}
          />
          <span>
            {ta('resetAlsoMfa')}
            <span className={styles.checkboxHint}>{ta('resetMfaHint')}</span>
          </span>
        </label>
        {failure === null ? null : (
          <p
            role="alert"
            className={cx(sharedStyles.banner, sharedStyles.bannerError)}
            data-testid="reset-error"
          >
            {failure}
          </p>
        )}
      </div>
    </ConfirmDialog>
  );
}
