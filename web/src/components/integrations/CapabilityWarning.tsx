import type { components } from '../../api/schema';
import { ti } from '../../i18n/messages.integrations';
import styles from './integrations.module.css';

type Warning = components['schemas']['WarningDto'];

/** The sentence for one capability warning (BR-14.8, BR-15.2). */
export function warningText(warning: Warning): string {
  switch (warning.code) {
    case 'tool_calling_unsupported':
      return ti('capabilityToolUnsupported');
    case 'tool_calling_unknown':
      return ti('capabilityToolUnknown');
    case 'context_too_small':
      return ti('capabilityContextSmall', {
        have: (warning.have ?? 0).toLocaleString(),
        need: (warning.need ?? 0).toLocaleString(),
      });
    default:
      return ti('capabilityContextUnknown');
  }
}

interface CapabilityWarningProps {
  warnings: Warning[];
  id?: string | undefined;
  testId?: string | undefined;
}

/** What a model lacks for a job, and that jobs will refuse to run on it. Nothing when it suits. */
export function CapabilityWarning({ warnings, id, testId }: CapabilityWarningProps) {
  if (warnings.length === 0) {
    return null;
  }
  return (
    <ul id={id} className={styles.warnings} data-testid={testId}>
      {warnings.map((warning) => (
        <li key={warning.code}>
          <span aria-hidden="true">⚠ </span>
          {warningText(warning)}
        </li>
      ))}
    </ul>
  );
}
