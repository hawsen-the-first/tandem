import { useMutation, useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { call, client } from '../api/client';
import { errorMessage, fieldErrors } from '../api/messages';
import { PasswordField } from '../components/PasswordField';
import { unmetRules } from '../components/passwordRules';
import { format, t } from '../i18n/messages';
import { publicMessages as p } from '../i18n/messages.public';
import { cx } from '../styles/cx';
import shared from '../styles/shared.module.css';
import { AuthCard, FormError, useFailureFocus } from './publicShared';

export function Component() {
  const [params] = useSearchParams();
  // Kept in memory and removed from the address bar at once (history and Referer).
  const [token] = useState(() => params.get('token'));
  const navigate = useNavigate();
  useEffect(() => {
    void navigate({ search: '' }, { replace: true });
  }, [navigate]);

  const check = useQuery({
    queryKey: ['password-reset', token],
    enabled: token !== null && token !== '',
    staleTime: Infinity,
    queryFn: () =>
      call(client.POST('/api/password-resets/validate', { body: { token: token ?? '' } })),
  });

  if (token === null || token === '' || check.isError) {
    return (
      <AuthCard title={p.linkInvalidTitle} testId="reset-page">
        <p role="alert" data-testid="reset-invalid">
          {t('errorInvalidLink')}
        </p>
      </AuthCard>
    );
  }
  if (check.isPending) {
    return (
      <p className={cx(shared.narrow, shared.muted)} role="status" data-testid="reset-checking">
        {p.resetChecking}
      </p>
    );
  }
  return <ResetForm token={token} name={check.data.display_name} />;
}

function ResetForm({ token, name }: { token: string; name: string }) {
  const navigate = useNavigate();
  const form = useRef<HTMLFormElement>(null);
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  const complete = useMutation({
    mutationFn: () =>
      call(
        client.POST('/api/password-resets/complete', {
          body: { token, new_password: password },
        }),
      ),
    onSuccess: () => {
      setPassword('');
      setConfirm('');
      void navigate('/sign-in', { replace: true, state: { notice: p.resetDone } });
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      setErrors(fields);
      setFailure(Object.keys(fields).length > 0 ? null : errorMessage(error));
      setPassword('');
      setConfirm('');
      setSignal((n) => n + 1);
    },
  });

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    const problems: Record<string, string> = {};
    if (unmetRules(password).length > 0) problems.new_password = p.validationPasswordRules;
    if (confirm !== password) problems.confirm = p.validationMismatch;
    setErrors(problems);
    setFailure(null);
    if (Object.keys(problems).length > 0) {
      setSignal((n) => n + 1);
      return;
    }
    complete.mutate();
  }

  return (
    <AuthCard title={p.resetTitle} testId="reset-page">
      <p>{format(p.resetIntro, { name })}</p>
      <form ref={form} onSubmit={submit} noValidate className={shared.stack}>
        <FormError message={failure} testId="reset-error" />
        <PasswordField
          label={p.fieldNewPassword}
          value={password}
          onChange={setPassword}
          autoComplete="new-password"
          showRules
          error={errors.new_password ?? errors.password}
          testId="reset-password"
        />
        <PasswordField
          label={p.fieldConfirmPassword}
          value={confirm}
          onChange={setConfirm}
          autoComplete="new-password"
          error={errors.confirm}
          testId="reset-confirm"
        />
        <button
          type="submit"
          className={shared.button}
          disabled={complete.isPending}
          data-testid="reset-submit"
        >
          {p.resetSubmit}
        </button>
      </form>
    </AuthCard>
  );
}
