import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { call, client } from '../../api/client';
import { errorMessage } from '../../api/messages';
import type { components } from '../../api/schema';
import { FormField } from '../../components/FormField';
import { useToast } from '../../components/Toast';
import { t } from '../../i18n/messages';
import { cx } from '../../styles/cx';
import sharedStyles from '../../styles/shared.module.css';
import styles from './admin.module.css';
import { ta } from './shared';

type Settings = components['schemas']['SecuritySettingsDto'];

// BR-10.1: the idle time runs from 15 minutes to 30 days, the total from 1 hour to 90 days.
const IDLE_MIN_MINUTES = 15;
const IDLE_MAX_MINUTES = 30 * 24 * 60;
const ABSOLUTE_MIN_HOURS = 1;
const ABSOLUTE_MAX_HOURS = 90 * 24;

interface FormValues {
  idleMinutes: string;
  absoluteHours: string;
  onlineCheck: boolean;
}

function toForm(settings: Settings): FormValues {
  return {
    idleMinutes: String(settings.idle_timeout_seconds / 60),
    absoluteHours: String(settings.absolute_duration_seconds / 3600),
    onlineCheck: settings.online_breach_check,
  };
}

function wholeNumber(text: string): number | null {
  return /^\d+$/.test(text.trim()) ? Number(text.trim()) : null;
}

export interface SettingsProblems {
  idle?: string;
  absolute?: string;
}

/** Client-side checks that mirror BR-10.1; the server validates again. */
export function validateSettings(values: FormValues): SettingsProblems {
  const problems: SettingsProblems = {};
  const idle = wholeNumber(values.idleMinutes);
  const absolute = wholeNumber(values.absoluteHours);
  if (idle === null || idle < IDLE_MIN_MINUTES || idle > IDLE_MAX_MINUTES) {
    problems.idle = ta('settingsIdleRange');
  }
  if (absolute === null || absolute < ABSOLUTE_MIN_HOURS || absolute > ABSOLUTE_MAX_HOURS) {
    problems.absolute = ta('settingsAbsoluteRange');
  }
  if (
    problems.idle === undefined &&
    problems.absolute === undefined &&
    idle !== null &&
    absolute !== null &&
    idle * 60 > absolute * 3600
  ) {
    problems.idle = ta('settingsIdleExceeds');
  }
  return problems;
}

function SettingsForm({ initial }: { initial: Settings }) {
  const queryClient = useQueryClient();
  const toast = useToast();
  const [baseline, setBaseline] = useState<FormValues>(toForm(initial));
  const [values, setValues] = useState<FormValues>(baseline);
  const [problems, setProblems] = useState<SettingsProblems>({});
  const [failure, setFailure] = useState<string | null>(null);

  const dirty =
    values.idleMinutes !== baseline.idleMinutes ||
    values.absoluteHours !== baseline.absoluteHours ||
    values.onlineCheck !== baseline.onlineCheck;

  const save = useMutation({
    mutationFn: (body: Settings) => call(client.PUT('/api/admin/settings', { body })),
    onSuccess: (saved) => {
      const next = toForm(saved);
      setBaseline(next);
      setValues(next);
      queryClient.setQueryData(['admin', 'settings'], saved);
      toast.show('success', ta('settingsSaved'));
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  return (
    <form
      className={cx(styles.formGrid, styles.settingsForm)}
      noValidate
      data-testid="settings-form"
      onSubmit={(event) => {
        event.preventDefault();
        setFailure(null);
        const found = validateSettings(values);
        setProblems(found);
        if (found.idle !== undefined || found.absolute !== undefined) {
          return;
        }
        save.mutate({
          idle_timeout_seconds: Number(values.idleMinutes.trim()) * 60,
          absolute_duration_seconds: Number(values.absoluteHours.trim()) * 3600,
          online_breach_check: values.onlineCheck,
        });
      }}
    >
      <FormField
        label={ta('settingsIdle')}
        hint={ta('settingsIdleHint')}
        error={problems.idle}
        required
        testId="settings-idle"
      >
        {(control) => (
          <input
            {...control}
            inputMode="numeric"
            value={values.idleMinutes}
            data-testid="settings-idle-input"
            onChange={(event) => {
              setValues({ ...values, idleMinutes: event.target.value });
            }}
          />
        )}
      </FormField>
      <FormField
        label={ta('settingsAbsolute')}
        hint={ta('settingsAbsoluteHint')}
        error={problems.absolute}
        required
        testId="settings-absolute"
      >
        {(control) => (
          <input
            {...control}
            inputMode="numeric"
            value={values.absoluteHours}
            data-testid="settings-absolute-input"
            onChange={(event) => {
              setValues({ ...values, absoluteHours: event.target.value });
            }}
          />
        )}
      </FormField>
      <label className={styles.checkboxRow}>
        <input
          type="checkbox"
          checked={values.onlineCheck}
          data-testid="settings-online-input"
          onChange={(event) => {
            setValues({ ...values, onlineCheck: event.target.checked });
          }}
        />
        <span>
          {ta('settingsOnline')}
          <span className={styles.checkboxHint}>{ta('settingsOnlineHint')}</span>
        </span>
      </label>
      {failure === null ? null : (
        <p
          role="alert"
          className={cx(sharedStyles.banner, sharedStyles.bannerError)}
          data-testid="settings-error"
        >
          {failure}
        </p>
      )}
      <div>
        <button
          type="submit"
          className={sharedStyles.button}
          disabled={!dirty || save.isPending}
          data-testid="settings-save"
        >
          {ta('settingsSave')}
        </button>
      </div>
    </form>
  );
}

export function Component() {
  const settings = useQuery({
    queryKey: ['admin', 'settings'],
    queryFn: () => call(client.GET('/api/admin/settings')),
  });

  return (
    <>
      <h1>{ta('settingsTitle')}</h1>
      <p className={sharedStyles.muted}>{ta('settingsIntro')}</p>
      {settings.isPending ? (
        <p role="status" data-testid="settings-loading">
          {t('loading')}
        </p>
      ) : settings.isError ? (
        <div
          role="alert"
          className={cx(sharedStyles.banner, sharedStyles.bannerError)}
          data-testid="settings-load-error"
        >
          <p>{ta('settingsLoadError')}</p>
          <p>{errorMessage(settings.error)}</p>
          <button
            type="button"
            className={sharedStyles.button}
            onClick={() => {
              void settings.refetch();
            }}
          >
            {t('retry')}
          </button>
        </div>
      ) : (
        <SettingsForm initial={settings.data} />
      )}
    </>
  );
}
