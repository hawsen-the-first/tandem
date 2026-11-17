import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect, useId, useRef, useState } from 'react';
import { call, client } from '../../../../api/client';
import { errorMessage, fieldErrors } from '../../../../api/messages';
import { FormField } from '../../../../components/FormField';
import { SecretField } from '../../../../components/integrations/SecretField';
import { UrlField } from '../../../../components/integrations/UrlField';
import { checkOutboundUrl } from '../../../../components/integrations/outboundRules';
import { useToast } from '../../../../components/Toast';
import { t } from '../../../../i18n/messages';
import { ti } from '../../../../i18n/messages.integrations';
import { cx } from '../../../../styles/cx';
import shared from '../../../../styles/shared.module.css';
import dialogStyles from '../../../../components/ConfirmDialog.module.css';
import type { Provider, ProviderRequest } from '../api';
import { DEFAULTS_KEY, PROVIDERS_KEY } from '../api';
import styles from '../integrations.module.css';
import type { ModelDraft, ModelProblems } from './ModelListEditor';
import { ModelListEditor, checkModels, modelDraftFrom } from './ModelListEditor';

interface Draft {
  name: string;
  baseUrl: string;
  apiKey: string;
  removeKey: boolean;
  models: ModelDraft[];
}

export interface ProviderProblems {
  name?: string;
  baseUrl?: string;
  apiKey?: string;
  models?: string;
  modelRows: Record<string, ModelProblems>;
}

function draftFrom(provider: Provider | null): Draft {
  return {
    name: provider?.name ?? '',
    baseUrl: provider?.base_url ?? '',
    apiKey: '',
    removeKey: false,
    models: provider?.models.map(modelDraftFrom) ?? [],
  };
}

/** Client checks for the provider form (BR-14.2, BR-14.3, BR-19); the server checks again. */
export function checkProvider(draft: Pick<Draft, 'name' | 'baseUrl' | 'models'>): ProviderProblems {
  const problems: ProviderProblems = { modelRows: checkModels(draft.models) };
  const name = draft.name.trim();
  if (name === '' || name.length > 60) {
    problems.name = ti('providerNameInvalid');
  }
  const url = checkOutboundUrl(draft.baseUrl);
  if (url !== null) {
    problems.baseUrl = url;
  }
  return problems;
}

function hasProblems(problems: ProviderProblems): boolean {
  return (
    problems.name !== undefined ||
    problems.baseUrl !== undefined ||
    problems.apiKey !== undefined ||
    problems.models !== undefined ||
    Object.keys(problems.modelRows).length > 0
  );
}

function requestFrom(draft: Draft): ProviderRequest {
  const body: ProviderRequest = {
    name: draft.name.trim(),
    base_url: draft.baseUrl.trim(),
    remove_api_key: draft.removeKey,
    models: draft.models.map((model) => ({
      id: model.id,
      name: model.name.trim(),
      label: model.label.trim(),
      declared_tool_calling: model.declaredTool === '' ? null : model.declaredTool,
      declared_context_tokens:
        model.declaredContext.trim() === '' ? null : Number(model.declaredContext.trim()),
    })),
  };
  // An empty key means "keep the saved one" (BR-20.3); it is never sent as an empty string.
  if (!draft.removeKey && draft.apiKey !== '') {
    body.api_key = draft.apiKey;
  }
  return body;
}

interface ProviderDialogProps {
  /** `null` adds a provider; a provider edits it. The dialog is shown while `open`. */
  provider: Provider | null;
  open: boolean;
  onClose: () => void;
}

/** Add or edit a provider: name, base URL, key and models, with errors beside each field. */
export function ProviderDialog({ provider, open, onClose }: ProviderDialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const queryClient = useQueryClient();
  const toast = useToast();
  const [draft, setDraft] = useState<Draft>(() => draftFrom(provider));
  const [problems, setProblems] = useState<ProviderProblems>({ modelRows: {} });
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (dialog === null) {
      return;
    }
    if (open && !dialog.open) {
      dialog.showModal();
    } else if (!open && dialog.open) {
      dialog.close();
    }
  }, [open]);

  const save = useMutation({
    mutationFn: (body: ProviderRequest) =>
      provider === null
        ? call(client.POST('/api/admin/providers', { body }))
        : call(
            client.PUT('/api/admin/providers/{id}', {
              params: { path: { id: provider.id } },
              body,
            }),
          ),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: PROVIDERS_KEY });
      void queryClient.invalidateQueries({ queryKey: DEFAULTS_KEY });
      toast.show('success', provider === null ? ti('providerAdded') : ti('providerSaved'));
      onClose();
    },
    onError: (error) => {
      const fields = fieldErrors(error);
      const found: ProviderProblems = { modelRows: {} };
      if (fields['name']) found.name = fields['name'];
      if (fields['base_url']) found.baseUrl = fields['base_url'];
      if (fields['api_key']) found.apiKey = fields['api_key'];
      if (fields['models']) found.models = fields['models'];
      setProblems(found);
      setFailure(errorMessage(error));
    },
  });

  return (
    <dialog
      ref={ref}
      className={cx(dialogStyles.dialog, styles.dialogWide)}
      aria-labelledby={titleId}
      data-testid="provider-dialog"
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <h2 id={titleId} className={dialogStyles.title}>
        {provider === null
          ? ti('providerAddTitle')
          : ti('providerEditTitle', { name: provider.name })}
      </h2>
      <form
        className={styles.form}
        noValidate
        onSubmit={(event) => {
          event.preventDefault();
          setFailure(null);
          const found = checkProvider(draft);
          setProblems(found);
          if (hasProblems(found)) {
            return;
          }
          const body = requestFrom(draft);
          // The key leaves the browser with this request and is not kept (BR-20.3).
          setDraft((current) => ({ ...current, apiKey: '' }));
          save.mutate(body);
        }}
      >
        {failure === null ? null : (
          <p
            role="alert"
            className={cx(shared.banner, shared.bannerError)}
            data-testid="provider-dialog-error"
          >
            {failure}
          </p>
        )}
        <FormField
          label={ti('providerName')}
          hint={ti('providerNameHint')}
          error={problems.name}
          required
          testId="provider-name"
        >
          {(control) => (
            <input
              {...control}
              value={draft.name}
              autoComplete="off"
              maxLength={120}
              data-testid="provider-name-input"
              onChange={(event) => {
                setDraft({ ...draft, name: event.target.value });
              }}
            />
          )}
        </FormField>
        <UrlField
          label={ti('providerBaseUrl')}
          value={draft.baseUrl}
          error={problems.baseUrl}
          required
          placeholder="https://api.example.com/v1"
          testId="provider-url"
          onChange={(baseUrl) => {
            setDraft({ ...draft, baseUrl });
          }}
        />
        <SecretField
          label={ti('providerKey')}
          hint={ti('providerKeyHint')}
          value={draft.apiKey}
          saved={provider?.api_key_saved ?? false}
          savedAt={provider?.api_key_saved_at}
          error={problems.apiKey}
          removal={{
            pending: draft.removeKey,
            onChange: (removeKey) => {
              setDraft((current) => ({ ...current, removeKey, apiKey: '' }));
            },
          }}
          testId="provider-key"
          onChange={(apiKey) => {
            setDraft({ ...draft, apiKey });
          }}
        />
        <ModelListEditor
          models={draft.models}
          problems={problems.modelRows}
          listError={problems.models}
          onChange={(models) => {
            setDraft({ ...draft, models });
          }}
        />
        <div className={shared.row}>
          <button
            type="submit"
            className={shared.button}
            disabled={save.isPending}
            data-testid="provider-save"
          >
            {ti('providerSave')}
          </button>
          <button
            type="button"
            className={cx(shared.button, shared.secondary)}
            data-testid="provider-cancel"
            onClick={onClose}
          >
            {t('cancel')}
          </button>
        </div>
      </form>
    </dialog>
  );
}
