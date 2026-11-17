import { useId } from 'react';
import { ti } from '../../i18n/messages.integrations';
import { MAX_HOSTS } from './outboundRules';
import styles from './integrations.module.css';

interface HostListEditorProps {
  hosts: string[];
  onChange: (hosts: string[]) => void;
  /** Problems keyed by row index (from `checkHosts` or the server). */
  errors?: Record<number, string> | undefined;
  testId: string;
}

/** A network allow-list: one host per row, each with its own label, error and remove button. */
export function HostListEditor({ hosts, onChange, errors = {}, testId }: HostListEditorProps) {
  const id = useId();
  return (
    <fieldset className={styles.fieldset} data-testid={testId}>
      <legend>{ti('hostsLabel')}</legend>
      {hosts.map((host, index) => {
        const n = index + 1;
        const error = errors[index];
        const errorId = `${id}-${String(index)}-error`;
        return (
          // Rows are positional: the index is the identity the labels and errors refer to.
          <div key={index} className={styles.hostRow}>
            <div className={styles.inputRow}>
              <input
                aria-label={ti('hostLabel', { n })}
                aria-invalid={error === undefined ? undefined : true}
                aria-describedby={error === undefined ? undefined : errorId}
                value={host}
                autoComplete="off"
                spellCheck={false}
                autoCapitalize="off"
                placeholder="api.example.com"
                data-testid={`${testId}-input-${String(index)}`}
                onChange={(event) => {
                  onChange(
                    hosts.map((current, at) => (at === index ? event.target.value : current)),
                  );
                }}
              />
              <button
                type="button"
                className={styles.smallButton}
                aria-label={ti('hostRemove', { n })}
                data-testid={`${testId}-remove-${String(index)}`}
                onClick={() => {
                  onChange(hosts.filter((_, at) => at !== index));
                }}
              >
                {ti('secretRemove')}
              </button>
            </div>
            {error === undefined ? null : (
              <p
                id={errorId}
                className={styles.rowError}
                data-testid={`${testId}-error-${String(index)}`}
              >
                <span aria-hidden="true">⚠ </span>
                {error}
              </p>
            )}
          </div>
        );
      })}
      <div>
        <button
          type="button"
          className={styles.smallButton}
          disabled={hosts.length >= MAX_HOSTS}
          data-testid={`${testId}-add`}
          onClick={() => {
            onChange([...hosts, '']);
          }}
        >
          {ti('hostAdd')}
        </button>
        {hosts.length >= MAX_HOSTS ? (
          <p className={styles.hint}>{ti('hostTooMany', { max: MAX_HOSTS })}</p>
        ) : null}
      </div>
    </fieldset>
  );
}
