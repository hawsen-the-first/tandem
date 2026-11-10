import { cx } from '../styles/cx';
import { useState } from 'react';
import { t } from '../i18n/messages';
import shared from '../styles/shared.module.css';
import styles from './CopyOnceSecret.module.css';

interface CopyOnceSecretProps {
  /** What is being shown, for example "Invitation link" or "Recovery codes". */
  label: string;
  /** One or more lines (a link is one line; recovery codes are several). */
  secret: string[];
  /** Called when the person confirms they have saved it; the caller must then drop the secret. */
  onDismiss: () => void;
  testId?: string;
}

async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/**
 * Shows a one-time secret. It must be explicitly acknowledged before it can be dismissed, and the
 * parent clears it from memory on dismissal.
 */
export function CopyOnceSecret({
  label,
  secret,
  onDismiss,
  testId = 'secret',
}: CopyOnceSecretProps) {
  const [copied, setCopied] = useState(false);
  const [acknowledged, setAcknowledged] = useState(false);
  return (
    <section className={styles.box} aria-label={label} data-testid={testId}>
      <h3 className={styles.title}>{label}</h3>
      <p className={cx(shared.banner, shared.bannerWarning)} role="note">
        {t('secretShownOnce')}
      </p>
      <pre className={styles.value} tabIndex={0} data-testid={`${testId}-value`}>
        {secret.join('\n')}
      </pre>
      <div className={shared.row}>
        <button
          type="button"
          className={cx(shared.button, shared.secondary)}
          data-testid={`${testId}-copy`}
          onClick={() => {
            void copyText(secret.join('\n')).then(setCopied);
          }}
        >
          {copied ? t('copied') : t('copy')}
        </button>
        <span role="status" className="visually-hidden">
          {copied ? t('copied') : ''}
        </span>
        <label className={styles.ack}>
          <input
            type="checkbox"
            checked={acknowledged}
            data-testid={`${testId}-ack`}
            onChange={(event) => {
              setAcknowledged(event.target.checked);
            }}
          />
          {t('secretSaved')}
        </label>
        <button
          type="button"
          className={shared.button}
          disabled={!acknowledged}
          data-testid={`${testId}-done`}
          onClick={onDismiss}
        >
          {t('secretDone')}
        </button>
      </div>
    </section>
  );
}
