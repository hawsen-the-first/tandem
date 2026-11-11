import { useId } from 'react';
import type { ReactNode } from 'react';
import styles from './FormField.module.css';

export interface FieldControlProps {
  id: string;
  'aria-describedby': string | undefined;
  'aria-invalid': true | undefined;
  'aria-required': true | undefined;
}

interface FormFieldProps {
  label: string;
  error?: string | undefined;
  hint?: string | undefined;
  required?: boolean | undefined;
  testId?: string | undefined;
  /** Renders the control; spread the given props onto it so the label, hint and error are linked. */
  children: (control: FieldControlProps) => ReactNode;
}

/** A label, a control, an optional hint and an error that is linked to the control and never colour-only. */
export function FormField({ label, error, hint, required, testId, children }: FormFieldProps) {
  const id = useId();
  const hintId = `${id}-hint`;
  const errorId = `${id}-error`;
  const describedBy =
    [hint === undefined ? null : hintId, error === undefined ? null : errorId]
      .filter((value) => value !== null)
      .join(' ') || undefined;
  return (
    <div className={styles.field} data-testid={testId}>
      <label htmlFor={id} className={styles.label}>
        {label}
        {required === true ? <span aria-hidden="true"> *</span> : null}
      </label>
      {children({
        id,
        'aria-describedby': describedBy,
        'aria-invalid': error === undefined ? undefined : true,
        'aria-required': required === true ? true : undefined,
      })}
      {hint === undefined ? null : (
        <p id={hintId} className={styles.hint}>
          {hint}
        </p>
      )}
      {error === undefined ? null : (
        <p
          id={errorId}
          className={styles.error}
          data-testid={testId ? `${testId}-error` : undefined}
        >
          <span aria-hidden="true">⚠ </span>
          {error}
        </p>
      )}
    </div>
  );
}
