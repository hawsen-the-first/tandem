import { useId } from 'react';
import { CapabilityWarning } from '../../../../components/integrations/CapabilityWarning';
import { toolLabel } from '../../../../components/integrations/TestResultView';
import { FormField } from '../../../../components/FormField';
import { ti } from '../../../../i18n/messages.integrations';
import type { Model } from '../api';
import componentStyles from '../../../../components/integrations/integrations.module.css';
import styles from '../integrations.module.css';

export type DeclaredTool = '' | 'supported' | 'unsupported' | 'unknown';

/** A model row as edited. `key` is stable across edits; `id` is the server's, for existing rows. */
export interface ModelDraft {
  key: string;
  id: string | null;
  name: string;
  label: string;
  declaredTool: DeclaredTool;
  declaredContext: string;
  /** The model as last saved, for its detected values and warnings. */
  saved: Model | null;
}

export interface ModelProblems {
  name?: string;
  context?: string;
}

let nextKey = 1;

export function newModelDraft(): ModelDraft {
  nextKey += 1;
  return {
    key: `new-${String(nextKey)}`,
    id: null,
    name: '',
    label: '',
    declaredTool: '',
    declaredContext: '',
    saved: null,
  };
}

export function modelDraftFrom(model: Model): ModelDraft {
  const declared = model.declared_tool_calling;
  return {
    key: model.id,
    id: model.id,
    name: model.name,
    label: model.label ?? '',
    declaredTool:
      declared === 'supported' || declared === 'unsupported' || declared === 'unknown'
        ? declared
        : '',
    declaredContext:
      model.declared_context_tokens === null || model.declared_context_tokens === undefined
        ? ''
        : String(model.declared_context_tokens),
    saved: model,
  };
}

const MAX_CONTEXT = 10_000_000;

/** Client checks for the model rows (form validation table); the server checks again. */
export function checkModels(models: ModelDraft[]): Record<string, ModelProblems> {
  const problems: Record<string, ModelProblems> = {};
  const seen = new Set<string>();
  for (const model of models) {
    const found: ModelProblems = {};
    const name = model.name.trim();
    if (name === '' || name.length > 200) {
      found.name = ti('modelNameInvalid');
    } else if (seen.has(name)) {
      found.name = ti('modelNameDuplicate');
    }
    seen.add(name);
    const context = model.declaredContext.trim();
    if (context !== '') {
      const value = /^\d+$/.test(context) ? Number(context) : NaN;
      if (!Number.isInteger(value) || value < 1 || value > MAX_CONTEXT) {
        found.context = ti('modelContextInvalid');
      }
    }
    if (found.name !== undefined || found.context !== undefined) {
      problems[model.key] = found;
    }
  }
  return problems;
}

interface ModelListEditorProps {
  models: ModelDraft[];
  onChange: (models: ModelDraft[]) => void;
  problems: Record<string, ModelProblems>;
  /** A problem with the list as a whole, from the server. */
  listError?: string | undefined;
}

/** Rows of models: name, label, and declared overrides for tool calling and context length. */
export function ModelListEditor({ models, onChange, problems, listError }: ModelListEditorProps) {
  const hintId = useId();
  const update = (key: string, change: Partial<ModelDraft>) => {
    onChange(models.map((model) => (model.key === key ? { ...model, ...change } : model)));
  };
  return (
    <fieldset
      className={componentStyles.fieldset}
      aria-describedby={hintId}
      data-testid="model-list"
    >
      <legend>{ti('modelsLegend')}</legend>
      <p id={hintId} className={componentStyles.hint}>
        {ti('modelsHint')}
      </p>
      {listError === undefined ? null : (
        <p className={componentStyles.rowError} data-testid="model-list-error">
          <span aria-hidden="true">⚠ </span>
          {listError}
        </p>
      )}
      {models.length === 0 ? <p className={componentStyles.hint}>{ti('modelsEmpty')}</p> : null}
      {models.map((model, index) => {
        const n = index + 1;
        const found = problems[model.key] ?? {};
        const detected = model.saved?.detected_tool_calling;
        return (
          <div
            key={model.key}
            role="group"
            aria-label={ti('modelRowLabel', { n })}
            className={styles.modelRow}
            data-testid="model-row"
          >
            <p className={styles.modelRowHead}>
              <span aria-hidden="true">{ti('modelRowLabel', { n })}</span>
              <button
                type="button"
                className={componentStyles.smallButton}
                aria-label={ti('modelRemove', { n })}
                data-testid={`model-remove-${String(index)}`}
                onClick={() => {
                  onChange(models.filter((candidate) => candidate.key !== model.key));
                }}
              >
                {ti('secretRemove')}
              </button>
            </p>
            <FormField
              label={ti('modelName')}
              error={found.name}
              required
              testId={`model-name-${String(index)}`}
            >
              {(control) => (
                <input
                  {...control}
                  value={model.name}
                  autoComplete="off"
                  spellCheck={false}
                  data-testid={`model-name-${String(index)}-input`}
                  onChange={(event) => {
                    update(model.key, { name: event.target.value });
                  }}
                />
              )}
            </FormField>
            <FormField label={ti('modelLabel')} testId={`model-label-${String(index)}`}>
              {(control) => (
                <input
                  {...control}
                  value={model.label}
                  autoComplete="off"
                  data-testid={`model-label-${String(index)}-input`}
                  onChange={(event) => {
                    update(model.key, { label: event.target.value });
                  }}
                />
              )}
            </FormField>
            <FormField label={ti('modelToolCalling')} testId={`model-tool-${String(index)}`}>
              {(control) => (
                <select
                  {...control}
                  className={styles.select}
                  value={model.declaredTool}
                  data-testid={`model-tool-${String(index)}-input`}
                  onChange={(event) => {
                    update(model.key, { declaredTool: event.target.value as DeclaredTool });
                  }}
                >
                  <option value="">
                    {ti('modelToolDetected', { value: toolLabel(detected) })}
                  </option>
                  <option value="supported">{ti('toolSupported')}</option>
                  <option value="unsupported">{ti('toolUnsupported')}</option>
                  <option value="unknown">{ti('toolUnknown')}</option>
                </select>
              )}
            </FormField>
            <FormField
              label={ti('modelContext')}
              hint={
                model.saved?.detected_context_tokens
                  ? `${ti('modelContextHint')} (${model.saved.detected_context_tokens.toLocaleString()})`
                  : ti('modelContextHint')
              }
              error={found.context}
              testId={`model-context-${String(index)}`}
            >
              {(control) => (
                <input
                  {...control}
                  inputMode="numeric"
                  value={model.declaredContext}
                  autoComplete="off"
                  data-testid={`model-context-${String(index)}-input`}
                  onChange={(event) => {
                    update(model.key, { declaredContext: event.target.value });
                  }}
                />
              )}
            </FormField>
            {model.saved === null ? null : (
              <div className={styles.modelRowFull}>
                {model.saved.disagrees_with_detection ? (
                  <p className={componentStyles.hint}>{ti('disagrees')}</p>
                ) : null}
                <CapabilityWarning warnings={model.saved.warnings} />
              </div>
            )}
          </div>
        );
      })}
      <div>
        <button
          type="button"
          className={componentStyles.smallButton}
          data-testid="model-add"
          onClick={() => {
            onChange([...models, newModelDraft()]);
          }}
        >
          {ti('modelsAdd')}
        </button>
      </div>
    </fieldset>
  );
}
