import { cx } from '../styles/cx';
import { useEffect, useId, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { t } from '../i18n/messages';
import shared from '../styles/shared.module.css';
import styles from './ConfirmDialog.module.css';

interface ConfirmDialogProps {
  open: boolean;
  title: string;
  children: ReactNode;
  confirmLabel: string;
  /** Styles the confirm button as destructive. */
  destructive?: boolean;
  /** Require this exact text to be typed before the confirm button is enabled. */
  requireTyped?: string;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
  testId?: string;
}

/**
 * A modal confirmation built on the native dialog element, which traps focus, closes on Escape,
 * makes the page behind it inert and returns focus to the control that opened it.
 */
export function ConfirmDialog({
  open,
  title,
  children,
  confirmLabel,
  destructive = false,
  requireTyped,
  busy = false,
  onConfirm,
  onCancel,
  testId = 'confirm-dialog',
}: ConfirmDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const [typed, setTyped] = useState('');

  useEffect(() => {
    const dialog = ref.current;
    if (dialog === null) {
      return;
    }
    if (open && !dialog.open) {
      setTyped('');
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  const confirmEnabled = !busy && (requireTyped === undefined || typed === requireTyped);

  return (
    <dialog
      ref={ref}
      className={styles.dialog}
      aria-labelledby={titleId}
      data-testid={testId}
      onCancel={(event) => {
        event.preventDefault();
        onCancel();
      }}
    >
      <h2 id={titleId} className={styles.title}>
        {title}
      </h2>
      <div className={styles.body}>{children}</div>
      {requireTyped === undefined ? null : (
        <label className={styles.typed}>
          <span>
            Type <strong>{requireTyped}</strong> to confirm
          </span>
          <input
            value={typed}
            autoComplete="off"
            data-testid={`${testId}-typed`}
            onChange={(event) => {
              setTyped(event.target.value);
            }}
          />
        </label>
      )}
      <div className={shared.row}>
        <button
          type="button"
          className={cx(shared.button, destructive && shared.danger)}
          disabled={!confirmEnabled}
          data-testid={`${testId}-confirm`}
          onClick={onConfirm}
        >
          {confirmLabel}
        </button>
        <button
          type="button"
          className={cx(shared.button, shared.secondary)}
          data-testid={`${testId}-cancel`}
          onClick={onCancel}
        >
          {t('cancel')}
        </button>
      </div>
    </dialog>
  );
}
