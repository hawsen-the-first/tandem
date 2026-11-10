import { useMutation } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { call, client } from '../../api/client';
import { errorMessage, fieldErrors } from '../../api/messages';
import { useToast } from '../../components/Toast';
import { PasswordField } from '../../components/PasswordField';
import { unmetRules } from '../../components/passwordRules';
import { publicMessages as p } from '../../i18n/messages.public';
import shared from '../../styles/shared.module.css';
import { FormError, useFailureFocus } from '../publicShared';

export function ChangePasswordForm() {
  const toast = useToast();
  const form = useRef<HTMLFormElement>(null);
  const [current, setCurrent] = useState('');
  const [next, setNext] = useState('');
  const [confirm, setConfirm] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  const change = useMutation({
    mutationFn: () =>
      call(
        client.POST('/api/auth/password', {
          body: { current_password: current, new_password: next },
        }),
      ),
    onSuccess: () => {
      setCurrent('');
      setNext('');
      setConfirm('');
      setErrors({});
      setFailure(null);
      toast.show('success', p.changePasswordDone);
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      setErrors({
        current: fields.current_password ?? fields.current ?? '',
        next: fields.new_password ?? fields.password ?? '',
      });
      setFailure(Object.keys(fields).length > 0 ? null : errorMessage(error));
      setCurrent('');
      setNext('');
      setConfirm('');
      setSignal((n) => n + 1);
    },
  });

  function submit(event: SyntheticEvent) {
    event.preventDefault();
    const problems: Record<string, string> = {};
    if (current === '') problems.current = p.validationCurrentPassword;
    if (unmetRules(next).length > 0) problems.next = p.validationPasswordRules;
    if (confirm !== next) problems.confirm = p.validationMismatch;
    setErrors(problems);
    setFailure(null);
    if (Object.keys(problems).length > 0) {
      setSignal((n) => n + 1);
      return;
    }
    change.mutate();
  }

  const shown = (key: string) => (errors[key] === '' ? undefined : errors[key]);

  return (
    <form
      ref={form}
      onSubmit={submit}
      noValidate
      className={shared.stack}
      aria-labelledby="change-password-title"
      data-testid="change-password-form"
    >
      <h2 id="change-password-title">{p.changePasswordTitle}</h2>
      <FormError message={failure} testId="change-password-error" />
      <PasswordField
        label={p.fieldCurrentPassword}
        value={current}
        onChange={setCurrent}
        autoComplete="current-password"
        error={shown('current')}
        testId="change-current"
      />
      <PasswordField
        label={p.fieldNewPassword}
        value={next}
        onChange={setNext}
        autoComplete="new-password"
        showRules
        error={shown('next')}
        testId="change-new"
      />
      <PasswordField
        label={p.fieldConfirmPassword}
        value={confirm}
        onChange={setConfirm}
        autoComplete="new-password"
        error={shown('confirm')}
        testId="change-confirm"
      />
      <div>
        <button
          type="submit"
          className={shared.button}
          disabled={change.isPending}
          data-testid="change-submit"
        >
          {p.changePasswordSubmit}
        </button>
      </div>
    </form>
  );
}
