import { useMutation } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { Link } from 'react-router';
import { call, client } from '../../api/client';
import { errorMessage, fieldErrors } from '../../api/messages';
import { isAdmin, useSession } from '../../auth/SessionProvider';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { CopyOnceSecret } from '../../components/CopyOnceSecret';
import { FormField } from '../../components/FormField';
import { PasswordField } from '../../components/PasswordField';
import { useToast } from '../../components/Toast';
import { format, t } from '../../i18n/messages';
import { publicMessages as p } from '../../i18n/messages.public';
import { cx } from '../../styles/cx';
import shared from '../../styles/shared.module.css';
import { FormError, useFailureFocus } from '../publicShared';

export function MfaSection() {
  const { me, refresh } = useSession();
  const toast = useToast();
  const form = useRef<HTMLFormElement>(null);
  const [confirmRegenerate, setConfirmRegenerate] = useState(false);
  const [newCodes, setNewCodes] = useState<string[] | null>(null);
  const [disabling, setDisabling] = useState(false);
  const [password, setPassword] = useState('');
  const [code, setCode] = useState('');
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [failure, setFailure] = useState<string | null>(null);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  const regenerate = useMutation({
    mutationFn: () => call(client.POST('/api/auth/mfa/recovery-codes')),
    onSuccess: (result) => {
      setConfirmRegenerate(false);
      setNewCodes(result.recovery_codes);
      setFailure(null);
    },
    onError: (error) => {
      setConfirmRegenerate(false);
      setFailure(errorMessage(error));
    },
  });

  const disable = useMutation({
    mutationFn: () =>
      call(client.POST('/api/auth/mfa/disable', { body: { password, code: code.trim() } })),
    onSuccess: async () => {
      setPassword('');
      setCode('');
      setDisabling(false);
      setFailure(null);
      await refresh();
      toast.show('success', p.mfaDisabled);
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      setErrors(fields);
      setFailure(Object.keys(fields).length > 0 ? null : errorMessage(error));
      setPassword('');
      setCode('');
      setSignal((n) => n + 1);
    },
  });

  function submitDisable(event: SyntheticEvent) {
    event.preventDefault();
    const problems: Record<string, string> = {};
    if (password === '') problems.password = p.validationPasswordMissing;
    if (code.trim() === '') problems.code = p.validationAnyCode;
    setErrors(problems);
    setFailure(null);
    if (Object.keys(problems).length > 0) {
      setSignal((n) => n + 1);
      return;
    }
    disable.mutate();
  }

  if (me === null) {
    return null;
  }
  const enrolled = me.mfa.enrolled;
  const admin = isAdmin(me);

  return (
    <section aria-labelledby="mfa-section-title" className={shared.stack} data-testid="mfa-section">
      <h2 id="mfa-section-title">{p.mfaSectionTitle}</h2>
      <p data-testid="mfa-status">
        {enrolled
          ? format(p.mfaStatusOn, { count: me.mfa.recovery_codes_remaining })
          : p.mfaStatusOff}
      </p>
      {admin ? (
        <p className={shared.muted} data-testid="mfa-required-note">
          {p.mfaRequiredNote}
        </p>
      ) : null}
      {enrolled && me.mfa.remind_regenerate ? (
        <p className={cx(shared.banner, shared.bannerWarning)} data-testid="mfa-remind">
          {p.mfaRemind}
        </p>
      ) : null}
      <FormError message={failure} testId="mfa-section-error" />

      {newCodes === null ? null : (
        <CopyOnceSecret
          label={p.mfaNewCodes}
          secret={newCodes}
          testId="regenerated"
          onDismiss={() => {
            setNewCodes(null);
            void refresh();
          }}
        />
      )}

      <div className={shared.row}>
        {enrolled ? null : (
          <Link to="/mfa/enrol" className={shared.button} data-testid="mfa-setup">
            {p.mfaSetUp}
          </Link>
        )}
        {enrolled ? (
          <button
            type="button"
            className={cx(shared.button, shared.secondary)}
            data-testid="mfa-regenerate"
            onClick={() => {
              setConfirmRegenerate(true);
            }}
          >
            {p.mfaRegenerate}
          </button>
        ) : null}
        {enrolled && !admin && !disabling ? (
          <button
            type="button"
            className={cx(shared.button, shared.secondary)}
            data-testid="mfa-disable"
            onClick={() => {
              setDisabling(true);
            }}
          >
            {p.mfaDisable}
          </button>
        ) : null}
      </div>

      {disabling ? (
        <form
          ref={form}
          onSubmit={submitDisable}
          noValidate
          className={shared.stack}
          aria-labelledby="mfa-disable-title"
          data-testid="mfa-disable-form"
        >
          <h3 id="mfa-disable-title">{p.mfaDisableTitle}</h3>
          <p>{p.mfaDisableIntro}</p>
          <PasswordField
            label={p.fieldPassword}
            value={password}
            onChange={setPassword}
            autoComplete="current-password"
            error={errors.password}
            testId="disable-password"
          />
          <FormField label={p.fieldCode} error={errors.code} required testId="disable-code">
            {(control) => (
              <input
                {...control}
                value={code}
                autoComplete="one-time-code"
                data-testid="disable-code-input"
                onChange={(event) => {
                  setCode(event.target.value);
                }}
              />
            )}
          </FormField>
          <div className={shared.row}>
            <button
              type="submit"
              className={cx(shared.button, shared.danger)}
              disabled={disable.isPending}
              data-testid="disable-submit"
            >
              {p.mfaDisableSubmit}
            </button>
            <button
              type="button"
              className={cx(shared.button, shared.secondary)}
              data-testid="disable-cancel"
              onClick={() => {
                setDisabling(false);
                setPassword('');
                setCode('');
                setErrors({});
              }}
            >
              {t('cancel')}
            </button>
          </div>
        </form>
      ) : null}

      <ConfirmDialog
        open={confirmRegenerate}
        title={p.mfaRegenerateTitle}
        confirmLabel={p.mfaRegenerateConfirm}
        busy={regenerate.isPending}
        testId="regenerate-dialog"
        onConfirm={() => {
          regenerate.mutate();
        }}
        onCancel={() => {
          setConfirmRegenerate(false);
        }}
      >
        <p>{p.mfaRegenerateBody}</p>
      </ConfirmDialog>
    </section>
  );
}
