import { useMutation } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { Navigate, useLocation, useNavigate } from 'react-router';
import { call, client } from '../api/client';
import { ApiError } from '../api/errors';
import { errorMessage } from '../api/messages';
import { useSession } from '../auth/SessionProvider';
import { FormField } from '../components/FormField';
import { PasswordField } from '../components/PasswordField';
import { throttleMessage } from '../i18n/messages';
import { publicMessages as p } from '../i18n/messages.public';
import { cx } from '../styles/cx';
import shared from '../styles/shared.module.css';
import { MfaChallengeForm } from './MfaChallengeForm';
import { AuthCard, FormError, emailProblem, useFailureFocus } from './publicShared';

interface LocationState {
  from?: string;
  notice?: string;
}

function secondsLeft(until: number | null): number {
  return until === null ? 0 : Math.max(0, Math.ceil((until - Date.now()) / 1000));
}

export function Component() {
  const { me, refresh } = useSession();
  const navigate = useNavigate();
  const location = useLocation();
  const state = (location.state ?? {}) as LocationState;
  const form = useRef<HTMLFormElement>(null);
  const [step, setStep] = useState<'credentials' | 'mfa'>('credentials');
  const [recovery, setRecovery] = useState(false);
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [emailError, setEmailError] = useState<string | undefined>(undefined);
  const [passwordError, setPasswordError] = useState<string | undefined>(undefined);
  const [failure, setFailure] = useState<string | null>(null);
  const [retryUntil, setRetryUntil] = useState<number | null>(null);
  const [remaining, setRemaining] = useState(0);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  useEffect(() => {
    if (retryUntil === null) {
      return;
    }
    const tick = () => {
      const left = secondsLeft(retryUntil);
      setRemaining(left);
      if (left === 0) {
        setRetryUntil(null);
      }
    };
    tick();
    const timer = window.setInterval(tick, 1000);
    return () => {
      window.clearInterval(timer);
    };
  }, [retryUntil]);

  const signIn = useMutation({
    mutationFn: (code: string | null) =>
      call(
        client.POST('/api/auth/sign-in', {
          body: {
            email: email.trim(),
            password,
            second_factor: code === null ? null : { kind: recovery ? 'recovery' : 'totp', code },
          },
        }),
      ),
    onSuccess: async (result) => {
      if (result.status === 'mfa_required') {
        setStep('mfa');
        setFailure(null);
        return;
      }
      setPassword('');
      await refresh();
      void navigate(state.from ?? '/', { replace: true });
    },
    onError: (error) => {
      // The password is not kept after a failed attempt; a second-factor failure starts over.
      setPassword('');
      setStep('credentials');
      setRecovery(false);
      if (error instanceof ApiError && error.code === 'rate_limited') {
        const seconds = error.retryAfterSeconds ?? 1;
        setRetryUntil(Date.now() + seconds * 1000);
        setRemaining(seconds);
        setFailure(null);
      } else {
        setFailure(errorMessage(error));
      }
      setSignal((n) => n + 1);
    },
  });

  if (me !== null) {
    return <Navigate to="/" replace />;
  }

  const throttled = retryUntil !== null && remaining > 0;

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    const emailIssue = emailProblem(email, p.validationEmail);
    const passwordIssue = password === '' ? p.validationPasswordMissing : undefined;
    setEmailError(emailIssue);
    setPasswordError(passwordIssue);
    setFailure(null);
    if (emailIssue !== undefined || passwordIssue !== undefined) {
      setSignal((n) => n + 1);
      return;
    }
    signIn.mutate(null);
  }

  if (step === 'mfa') {
    return (
      <main className={cx(shared.card, shared.narrow)} data-testid="signin-page">
        <MfaChallengeForm
          recovery={recovery}
          busy={signIn.isPending}
          error={failure}
          onToggleRecovery={() => {
            setRecovery((current) => !current);
          }}
          onSubmit={(code) => {
            signIn.mutate(code);
          }}
          onBack={() => {
            setPassword('');
            setStep('credentials');
            setRecovery(false);
          }}
        />
      </main>
    );
  }

  return (
    <AuthCard title={p.signInTitle} testId="signin-page">
      {state.notice === undefined ? null : (
        <p className={shared.banner} role="status" data-testid="signin-notice">
          {state.notice}
        </p>
      )}
      <form ref={form} onSubmit={submit} noValidate className={shared.stack}>
        <FormError message={failure} testId="signin-error" />
        {throttled ? (
          <p
            role="timer"
            className={cx(shared.banner, shared.bannerError)}
            data-testid="signin-throttle"
          >
            {throttleMessage(remaining)}
          </p>
        ) : null}
        <FormField label={p.fieldEmail} error={emailError} required testId="signin-email">
          {(control) => (
            <input
              {...control}
              type="email"
              value={email}
              autoComplete="username"
              data-testid="signin-email-input"
              onChange={(event) => {
                setEmail(event.target.value);
              }}
            />
          )}
        </FormField>
        <PasswordField
          label={p.fieldPassword}
          value={password}
          onChange={setPassword}
          autoComplete="current-password"
          error={passwordError}
          testId="signin-password"
        />
        <button
          type="submit"
          className={shared.button}
          disabled={signIn.isPending || throttled}
          data-testid="signin-submit"
        >
          {p.signInSubmit}
        </button>
      </form>
    </AuthCard>
  );
}
