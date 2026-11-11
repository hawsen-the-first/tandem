import { format } from '../../i18n/messages';
import { adminMessages } from '../../i18n/messages.admin';
import type { AdminMessageKey } from '../../i18n/messages.admin';

/** Look up an administration message, filling `{placeholders}`. */
export function ta(key: AdminMessageKey, values?: Record<string, string | number>): string {
  return format(adminMessages[key], values);
}

/** A moment in the reader's locale, with the exact ISO value kept for machines and assistive tech. */
export function FormattedTime({ iso }: { iso: string }) {
  return <time dateTime={iso}>{new Date(iso).toLocaleString()}</time>;
}

export function roleLabel(role: string): string {
  return role === 'admin' ? ta('roleAdmin') : ta('roleMember');
}
