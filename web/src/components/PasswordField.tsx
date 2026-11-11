import { useId, useState } from 'react';
import { t } from '../i18n/messages';
import { FormField } from './FormField';
import styles from './PasswordField.module.css';
import { PASSWORD_RULES } from './passwordRules';

interface PasswordFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoComplete: 'new-password' | 'current-password';
  /** Show the live checklist of password requirements (for choosing a new password). */
  showRules?: boolean;
  error?: string | undefined;
  testId?: string;
}

/** A password input with a show/hide toggle and, when choosing a new password, a live checklist. */
export function PasswordField({
  label,
  value,
  onChange,
  autoComplete,
  showRules = false,
  error,
  testId = 'password',
}: PasswordFieldProps) {
  const [revealed, setRevealed] = useState(false);
  const rulesId = useId();
  return (
    <div className={styles.wrapper}>
      <FormField label={label} error={error} required testId={testId}>
        {(control) => (
          <div className={styles.inputRow}>
            <input
              {...control}
              type={revealed ? 'text' : 'password'}
              value={value}
              autoComplete={autoComplete}
              spellCheck={false}
              autoCapitalize="off"
              data-testid={`${testId}-input`}
              aria-describedby={
                [control['aria-describedby'], showRules ? rulesId : undefined]
                  .filter((part) => part !== undefined)
                  .join(' ') || undefined
              }
              onChange={(event) => {
                onChange(event.target.value);
              }}
            />
            <button
              type="button"
              className={styles.toggle}
              aria-pressed={revealed}
              data-testid={`${testId}-toggle`}
              onClick={() => {
                setRevealed((current) => !current);
              }}
            >
              {revealed ? t('passwordHide') : t('passwordShow')}
            </button>
          </div>
        )}
      </FormField>
      {showRules ? (
        <div id={rulesId} className={styles.rules} data-testid={`${testId}-rules`}>
          <p className={styles.rulesTitle}>{t('passwordRulesTitle')}</p>
          <ul>
            {PASSWORD_RULES.map((rule) => {
              const met = rule.met(value);
              return (
                <li key={rule.id} data-testid={`${testId}-rule-${rule.id}`} data-met={met}>
                  <span aria-hidden="true">{met ? '✓ ' : '○ '}</span>
                  {t(rule.label)}
                  <span className="visually-hidden">
                    {' '}
                    ({met ? t('passwordRuleMet') : t('passwordRuleUnmet')})
                  </span>
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
    </div>
  );
}
