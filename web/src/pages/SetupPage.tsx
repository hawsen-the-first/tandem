import { useMutation, useQuery } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { useNavigate } from 'react-router';
import { call, client } from '../api/client';
import { errorMessage, fieldErrors } from '../api/messages';
import { useSession } from '../auth/SessionProvider';
import { FormField } from '../components/FormField';
import { PasswordField } from '../components/PasswordField';
import { unmetRules } from '../components/passwordRules';
import { t } from '../i18n/messages';
import { publicMessages as p } from '../i18n/messages.public';
import { cx } from '../styles/cx';
import shared from '../styles/shared.module.css';
import NotFoundPage from './NotFoundPage';
import { AuthCard, FormError, emailProblem, nameProblem, useFailureFocus } from './publicShared';

export function Component() {
  const status = useQuery({
    queryKey: ['setup-status'],
    queryFn: () => call(client.GET('/api/setup')),
    staleTime: 0,
  });
  if (status.isPending) {
    return (
      <p className={cx(shared.narrow, shared.muted)} role="status">
        {t('loading')}
      </p>
    );
  }
  if (status.isError) {
    return (
      <AuthCard title={p.setupTitle} testId="setup-page">
        <FormError message={errorMessage(status.error)} testId="setup-error" />
      </AuthCard>
    );
  }
  if (!status.data.available) {
    return <NotFoundPage />;
  }
  return <SetupForm />;
}

function SetupForm() {
  const { refresh } = useSession();
  const navigate = useNavigate();
  const form = useRef<HTMLFormElement>(null);
  const [displayName, setDisplayName] = useState('');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  const create = useMutation({
    mutationFn: () =>
      call(
        client.POST('/api/setup', {
          body: { display_name: displayName.trim(), email: email.trim(), password },
        }),
      ),
    onSuccess: async () => {
      await refresh();
      void navigate('/mfa/enrol', { replace: true });
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
    const emailIssue = emailProblem(email, p.validationEmail);
    if (nameIssue !== undefined) problems.display_name = nameIssue;
    if (emailIssue !== undefined) problems.email = emailIssue;
    if (unmetRules(password).length > 0) problems.password = p.validationPasswordRules;
    if (confirm !== password) problems.confirm = p.validationMismatch;
    setErrors(problems);
    setFailure(null);
    if (Object.keys(problems).length > 0) {
      setSignal((n) => n + 1);
      return;
    }
    create.mutate();
  }

  return (
    <AuthCard title={p.setupTitle} testId="setup-page">
      <p>{p.setupIntro}</p>
      <form ref={form} onSubmit={submit} noValidate className={shared.stack}>
        <FormError message={failure} testId="setup-error" />
        <FormField label={p.fieldName} error={errors.display_name} required testId="setup-name">
          {(control) => (
            <input
              {...control}
              value={displayName}
              autoComplete="name"
              data-testid="setup-name-input"
              onChange={(event) => {
                setDisplayName(event.target.value);
              }}
            />
          )}
        </FormField>
        <FormField label={p.fieldEmail} error={errors.email} required testId="setup-email">
          {(control) => (
            <input
              {...control}
              type="email"
              value={email}
              autoComplete="email"
              data-testid="setup-email-input"
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
          autoComplete="new-password"
          showRules
          error={errors.password}
          testId="setup-password"
        />
        <PasswordField
          label={p.fieldConfirmPassword}
          value={confirm}
          onChange={setConfirm}
          autoComplete="new-password"
          error={errors.confirm}
          testId="setup-confirm"
        />
        <button
          type="submit"
          className={shared.button}
          disabled={create.isPending}
          data-testid="setup-submit"
        >
          {create.isPending ? t('loading') : p.setupSubmit}
        </button>
      </form>
    </AuthCard>
  );
}
