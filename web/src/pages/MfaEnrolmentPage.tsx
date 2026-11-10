import { useMutation } from '@tanstack/react-query';
import QRCode from 'qrcode';
import { useEffect, useRef, useState } from 'react';
import type { SyntheticEvent } from 'react';
import { Link, useNavigate } from 'react-router';
import { call, client } from '../api/client';
import { errorMessage, fieldErrors } from '../api/messages';
import { isAdmin, useSession } from '../auth/SessionProvider';
import { CopyOnceSecret } from '../components/CopyOnceSecret';
import { FormField } from '../components/FormField';
import { t } from '../i18n/messages';
import { publicMessages as p } from '../i18n/messages.public';
import shared from '../styles/shared.module.css';
import { cx } from '../styles/cx';
import { AuthCard, FormError, totpProblem, useFailureFocus } from './publicShared';

type Stage = 'intro' | 'scan' | 'verify' | 'codes';

interface Enrolment {
  secret: string;
  uri: string;
}

function downloadCodes(codes: string[]): void {
  const blob = new Blob([`Tandem recovery codes\n\n${codes.join('\n')}\n`], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const link = document.createElement('a');
  link.href = url;
  link.download = 'tandem-recovery-codes.txt';
  link.click();
  URL.revokeObjectURL(url);
}

export function Component() {
  const { me, refresh, signOut } = useSession();
  const navigate = useNavigate();
  const form = useRef<HTMLFormElement>(null);
  const [stage, setStage] = useState<Stage>('intro');
  const [enrolment, setEnrolment] = useState<Enrolment | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  const [code, setCode] = useState('');
  const [codeError, setCodeError] = useState<string | undefined>(undefined);
  const [failure, setFailure] = useState<string | null>(null);
  const [recoveryCodes, setRecoveryCodes] = useState<string[] | null>(null);
  const [signal, setSignal] = useState(0);
  useFailureFocus(form, signal);

  useEffect(() => {
    if (enrolment === null) {
      return;
    }
    let current = true;
    void QRCode.toString(enrolment.uri, { type: 'svg', margin: 2, errorCorrectionLevel: 'M' }).then(
      (svg) => {
        if (current) {
          setQr(`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`);
        }
      },
    );
    return () => {
      current = false;
    };
  }, [enrolment]);

  const start = useMutation({
    mutationFn: () => call(client.POST('/api/auth/mfa/enrol')),
    onSuccess: (result) => {
      setEnrolment({ secret: result.secret_base32, uri: result.otpauth_uri });
      setFailure(null);
      setStage('scan');
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  const confirm = useMutation({
    mutationFn: () => call(client.POST('/api/auth/mfa/confirm', { body: { code: code.trim() } })),
    onSuccess: (result) => {
      setRecoveryCodes(result.recovery_codes);
      setEnrolment(null);
      setQr(null);
      setCode('');
      setFailure(null);
      setStage('codes');
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      setCodeError(fields.code ?? undefined);
      setFailure(fields.code === undefined ? errorMessage(error) : null);
      setCode('');
      setSignal((n) => n + 1);
    },
  });

  function submitCode(event: SyntheticEvent) {
    event.preventDefault();
    const issue = totpProblem(code, p.validationTotp);
    setCodeError(issue);
    setFailure(null);
    if (issue !== undefined) {
      setSignal((n) => n + 1);
      return;
    }
    confirm.mutate();
  }

  async function finish() {
    setRecoveryCodes(null);
    await refresh();
    void navigate('/', { replace: true });
  }

  const required = isAdmin(me);
  const alreadyEnrolled = me?.mfa.enrolled === true && stage === 'intro';

  return (
    <AuthCard title={p.mfaTitle} testId="mfa-page">
      {stage === 'intro' ? (
        <div className={shared.stack}>
          {alreadyEnrolled ? (
            <>
              <p data-testid="mfa-already">{p.mfaAlready}</p>
              <Link to="/" className={shared.button}>
                {p.mfaFinish}
              </Link>
            </>
          ) : (
            <>
              <p>{p.mfaIntro}</p>
              {required ? (
                <p className={cx(shared.banner, shared.bannerWarning)} data-testid="mfa-required">
                  {p.mfaRequiredNote}
                </p>
              ) : null}
              <FormError message={failure} testId="mfa-error" />
              <div className={shared.row}>
                <button
                  type="button"
                  className={shared.button}
                  disabled={start.isPending}
                  data-testid="mfa-start"
                  onClick={() => {
                    start.mutate();
                  }}
                >
                  {p.mfaStart}
                </button>
                {required ? null : (
                  <Link
                    to="/"
                    className={cx(shared.button, shared.secondary)}
                    data-testid="mfa-skip"
                  >
                    {p.mfaNotNow}
                  </Link>
                )}
              </div>
            </>
          )}
        </div>
      ) : null}

      {stage === 'scan' && enrolment !== null ? (
        <div className={shared.stack} data-testid="mfa-scan">
          <h2>{p.mfaScanTitle}</h2>
          <p>{p.mfaScanIntro}</p>
          {qr === null ? null : (
            <img src={qr} alt={p.mfaQrAlt} width={220} height={220} data-testid="mfa-qr" />
          )}
          <p>
            <strong>{p.mfaSecretLabel}</strong>
          </p>
          <code data-testid="mfa-secret" tabIndex={0}>
            {enrolment.secret}
          </code>
          <button
            type="button"
            className={shared.button}
            data-testid="mfa-next"
            onClick={() => {
              setStage('verify');
            }}
          >
            {p.mfaNext}
          </button>
        </div>
      ) : null}

      {stage === 'verify' ? (
        <form ref={form} onSubmit={submitCode} noValidate className={shared.stack}>
          <h2>{p.mfaVerifyTitle}</h2>
          <p>{p.mfaVerifyIntro}</p>
          <FormError message={failure} testId="mfa-error" />
          <FormField label={p.fieldCode} error={codeError} required testId="mfa-code">
            {(control) => (
              <input
                {...control}
                value={code}
                inputMode="numeric"
                autoComplete="one-time-code"
                autoFocus
                data-testid="mfa-code-input"
                onChange={(event) => {
                  setCode(event.target.value);
                }}
              />
            )}
          </FormField>
          <button
            type="submit"
            className={shared.button}
            disabled={confirm.isPending}
            data-testid="mfa-confirm"
          >
            {p.mfaVerifySubmit}
          </button>
        </form>
      ) : null}

      {stage === 'codes' && recoveryCodes !== null ? (
        <div className={shared.stack} data-testid="mfa-codes">
          <h2>{p.mfaCodesHeading}</h2>
          <p>{p.mfaCodesIntro}</p>
          <CopyOnceSecret
            label={p.mfaCodesTitle}
            secret={recoveryCodes}
            testId="recovery"
            onDismiss={() => {
              void finish();
            }}
          />
          <button
            type="button"
            className={cx(shared.button, shared.secondary)}
            data-testid="mfa-download"
            onClick={() => {
              downloadCodes(recoveryCodes);
            }}
          >
            {p.mfaDownload}
          </button>
        </div>
      ) : null}

      {stage !== 'codes' ? (
        <p>
          <button
            type="button"
            className={shared.linkButton}
            data-testid="mfa-sign-out"
            onClick={() => {
              void signOut();
            }}
          >
            {t('signOut')}
          </button>
        </p>
      ) : null}
    </AuthCard>
  );
}
