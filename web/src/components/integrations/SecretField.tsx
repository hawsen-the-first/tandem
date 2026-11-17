import { cx } from '../../styles/cx';
import { FormField } from '../FormField';
import { ti } from '../../i18n/messages.integrations';
import styles from './integrations.module.css';

interface SecretFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** A value is stored on the server. */
  saved: boolean;
  savedAt?: string | null | undefined;
  hint?: string | undefined;
  error?: string | undefined;
  required?: boolean | undefined;
  /** When given, a "Remove" button marks the saved value for removal on save (BR-20.3). */
  removal?: { pending: boolean; onChange: (pending: boolean) => void } | undefined;
  testId: string;
}

function savedText(saved: boolean, savedAt: string | null | undefined): string {
  if (!saved) {
    return ti('secretNotSaved');
  }
  return ti('secretSavedOn', {
    date: savedAt ? new Date(savedAt).toLocaleString() : '',
  }).trim();
}

/**
 * A write-only field for a key or token. It shows whether a value is saved and when, never the
 * value; leaving it empty keeps the saved one. The parent clears it after submitting.
 */
export function SecretField({
  label,
  value,
  onChange,
  saved,
  savedAt,
  hint,
  error,
  required,
  removal,
  testId,
}: SecretFieldProps) {
  const removing = removal?.pending === true;
  const fullHint = [saved && !removing ? ti('secretKeepHint') : null, hint ?? null]
    .filter((part) => part !== null)
    .join(' ');
  return (
    <FormField
      label={label}
      hint={fullHint === '' ? undefined : fullHint}
      error={error}
      required={required}
      testId={testId}
    >
      {(control) => (
        <>
          <p
            id={`${control.id}-status`}
            className={styles.secretStatus}
            data-testid={`${testId}-status`}
          >
            {removing ? ti('secretWillRemove') : savedText(saved, savedAt)}
          </p>
          <div className={styles.inputRow}>
            <input
              {...control}
              aria-describedby={[`${control.id}-status`, control['aria-describedby']]
                .filter((part) => part !== undefined)
                .join(' ')}
              type="password"
              value={value}
              autoComplete="off"
              spellCheck={false}
              autoCapitalize="off"
              disabled={removing}
              data-testid={`${testId}-input`}
              onChange={(event) => {
                onChange(event.target.value);
              }}
            />
            {removal !== undefined && saved ? (
              <button
                type="button"
                className={cx(styles.smallButton, !removing && styles.smallDanger)}
                aria-pressed={removing}
                data-testid={`${testId}-remove`}
                onClick={() => {
                  if (!removing) {
                    onChange('');
                  }
                  removal.onChange(!removing);
                }}
              >
                {removing ? ti('secretUndoRemove') : ti('secretRemove')}
              </button>
            ) : null}
          </div>
        </>
      )}
    </FormField>
  );
}
