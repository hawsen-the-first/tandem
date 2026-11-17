import { FormField } from '../FormField';
import { ti } from '../../i18n/messages.integrations';

interface UrlFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  /** The client-side reason (from `checkOutboundUrl`) or the server's, shown beside the field. */
  error?: string | undefined;
  hint?: string | undefined;
  required?: boolean | undefined;
  placeholder?: string | undefined;
  testId: string;
}

/** A base URL input. Its error carries the BR-19 reason; the server decides. */
export function UrlField({
  label,
  value,
  onChange,
  error,
  hint = ti('urlHint'),
  required,
  placeholder,
  testId,
}: UrlFieldProps) {
  return (
    <FormField label={label} hint={hint} error={error} required={required} testId={testId}>
      {(control) => (
        <input
          {...control}
          type="url"
          inputMode="url"
          autoComplete="off"
          spellCheck={false}
          autoCapitalize="off"
          placeholder={placeholder}
          value={value}
          data-testid={`${testId}-input`}
          onChange={(event) => {
            onChange(event.target.value);
          }}
        />
      )}
    </FormField>
  );
}
