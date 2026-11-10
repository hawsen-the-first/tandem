import { useMutation, useQuery } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { useNavigate, useSearchParams } from 'react-router';
import { call, client } from '../api/client';
import { errorMessage, fieldErrors } from '../api/messages';
import { useSession } from '../auth/SessionProvider';
import { FormField } from '../components/FormField';
import { PasswordField } from '../components/PasswordField';
import { unmetRules } from '../components/passwordRules';
import { format, t } from '../i18n/messages';
import { publicMessages as p } from '../i18n/messages.public';
import { cx } from '../styles/cx';
import shared from '../styles/shared.module.css';
import { AuthCard, FormError, nameProblem, useFailureFocus } from './publicShared';

export function Component() {
  const [params] = useSearchParams();
  // The token is kept in memory and removed from the address bar at once, so it does not stay in
  // history or in a Referer header.
  const [token] = useState(() => params.get('token'));
  const navigate = useNavigate();
  useEffect(() => {
    void navigate({ search: '' }, { replace: true });
  }, [navigate]);

  const check = useQuery({
    queryKey: ['invitation', token],
    enabled: token !== null && token !== '',
    staleTime: Infinity,
    queryFn: () => call(client.POST('/api/invitations/validate', { body: { token: token ?? '' } })),
  });

  if (token === null || token === '' || check.isError) {
    return (
      <AuthCard title={p.linkInvalidTitle} testId="invite-page">
        <p role="alert" data-testid="invite-invalid">
          {t('errorInvalidLink')}
        </p>
      </AuthCard>
    );
  }
  if (check.isPending) {
    return (
      <p className={cx(shared.narrow, shared.muted)} role="status" data-testid="invite-checking">
        {p.inviteChecking}
      </p>
    );
  }
  return <AcceptForm token={token} email={check.data.email} />;
}

function AcceptForm({ token, email }: { token: string; email: string }) {
  const { refresh } = useSession();
  const navigate = useNavigate();
  const form = useRef<HTMLFormElement>(null);
  const [displayName, setDisplayName] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  const accept = useMutation({
    mutationFn: () =>
      call(
        client.POST('/api/invitations/accept', {
          body: { token, display_name: displayName.trim(), password },
        }),
      ),
    onSuccess: async () => {
      await refresh();
      void navigate('/', { replace: true });
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
    const nameIssue = nameProblem(displayName, p.validationName);
    if (nameIssue !== undefined) problems.display_name = nameIssue;
    if (unmetRules(password).length > 0) problems.password = p.validationPasswordRules;
    if (confirm !== password) problems.confirm = p.validationMismatch;
    setErrors(problems);
    setFailure(null);
    if (Object.keys(problems).length > 0) {
      setSignal((n) => n + 1);
      return;
    }
    accept.mutate();
  }

  return (
    <AuthCard title={p.inviteTitle} testId="invite-page">
      <p>{format(p.inviteIntro, { email })}</p>
      <form ref={form} onSubmit={submit} noValidate className={shared.stack}>
        <FormError message={failure} testId="invite-error" />
        <FormField label={p.fieldName} error={errors.display_name} required testId="invite-name">
          {(control) => (
            <input
              {...control}
              value={displayName}
              autoComplete="name"
              data-testid="invite-name-input"
              onChange={(event) => {
                setDisplayName(event.target.value);
              }}
            />
          )}
        </FormField>
        <PasswordField
          label={p.fieldPassword}
          value={password}
          onChange={setPassword}
          autoComplete="new-password"
          showRules
          error={errors.password}
          testId="invite-password"
        />
        <PasswordField
          label={p.fieldConfirmPassword}
          value={confirm}
          onChange={setConfirm}
          autoComplete="new-password"
          error={errors.confirm}
          testId="invite-confirm"
        />
        <button
          type="submit"
          className={shared.button}
          disabled={accept.isPending}
          data-testid="invite-submit"
        >
          {p.inviteSubmit}
        </button>
      </form>
    </AuthCard>
  );
}
