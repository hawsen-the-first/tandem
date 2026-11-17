import { cx } from '../../styles/cx';
import { ti } from '../../i18n/messages.integrations';
import styles from './integrations.module.css';

export type BadgeStatus = 'ok' | 'warning' | 'error' | 'unknown' | 'running';

const ICONS: Record<BadgeStatus, string> = {
  ok: '✓',
  warning: '!',
  error: '✕',
  unknown: '?',
  running: '…',
};

const LABELS = {
  ok: 'statusOk',
  warning: 'statusWarning',
  error: 'statusError',
  unknown: 'statusUnknown',
  running: 'statusRunning',
} as const;

interface StatusBadgeProps {
  status: BadgeStatus;
  /** Text to show instead of the status name. */
  label?: string | undefined;
  testId?: string | undefined;
}

/** A status shown as text and an icon, never by colour alone. */
export function StatusBadge({ status, label, testId }: StatusBadgeProps) {
  return (
    <span
      className={cx(styles.badge, styles[`badge_${status}`])}
      data-status={status}
      data-testid={testId}
    >
      <span aria-hidden="true">{ICONS[status]}</span>
      {label ?? ti(LABELS[status])}
    </span>
  );
}
