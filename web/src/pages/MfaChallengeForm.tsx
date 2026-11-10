import { useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { FormField } from '../components/FormField';
import { publicMessages as p } from '../i18n/messages.public';
import shared from '../styles/shared.module.css';
import { FormError, totpProblem, useFailureFocus } from './publicShared';

interface MfaChallengeFormProps {
  recovery: boolean;
  busy: boolean;
  error: string | null;
  onToggleRecovery: () => void;
  onSubmit: (code: string) => void;
  onBack: () => void;
}

/** The second step of sign-in: an authenticator code, or one recovery code. */
export function MfaChallengeForm({
  recovery,
  busy,
  error,
  onToggleRecovery,
  onSubmit,
  onBack,
}: MfaChallengeFormProps) {
  const form = useRef<HTMLFormElement>(null);
  const [code, setCode] = useState('');
  const [problem, setProblem] = useState<string | undefined>(undefined);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    const issue = recovery
      ? code.trim() === ''
        ? p.validationRecovery
        : undefined
      : totpProblem(code, p.validationTotp);
    setProblem(issue);
    if (issue !== undefined) {
      setSignal((n) => n + 1);
      return;
    }
    onSubmit(code.trim());
  }

  return (
    <form ref={form} onSubmit={submit} noValidate className={shared.stack} data-testid="mfa-form">
      <h1>{p.signInMfaTitle}</h1>
      <p>{recovery ? p.signInMfaRecoveryIntro : p.signInMfaIntro}</p>
      <FormError message={error} testId="signin-error" />
      <FormField
        label={recovery ? p.fieldRecoveryCode : p.fieldCode}
        error={problem}
        required
        testId="mfa-code"
      >
        {(control) => (
          <input
            {...control}
            key={recovery ? 'recovery' : 'totp'}
            value={code}
            inputMode={recovery ? 'text' : 'numeric'}
            autoComplete="one-time-code"
            autoFocus
            data-testid="mfa-code-input"
            onChange={(event) => {
              setCode(event.target.value);
            }}
          />
        )}
      </FormField>
      <button type="submit" className={shared.button} disabled={busy} data-testid="mfa-submit">
        {p.signInMfaSubmit}
      </button>
      <div className={shared.row}>
        <button
          type="button"
          className={shared.linkButton}
          data-testid="mfa-toggle-recovery"
          onClick={() => {
            setCode('');
            setProblem(undefined);
            onToggleRecovery();
          }}
        >
          {recovery ? p.signInUseApp : p.signInUseRecovery}
        </button>
        <button type="button" className={shared.linkButton} data-testid="mfa-back" onClick={onBack}>
          {p.signInBack}
        </button>
      </div>
    </form>
  );
}
