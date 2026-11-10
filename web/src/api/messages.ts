import { t, throttleMessage } from '../i18n/messages';
import { ApiError } from './errors';

/** The message to show a person for any failure (business-rules.md, Error handling). */
export function errorMessage(error: unknown): string {
  if (!(error instanceof ApiError)) {
    return t('errorUnexpected');
  }
  switch (error.code) {
    case 'invalid_credentials':
      return t('errorInvalidCredentials');
    case 'rate_limited':
      return throttleMessage(error.retryAfterSeconds ?? 1);
    case 'invalid_link':
      return t('errorInvalidLink');
    case 'not_found':
      return t('errorNotFound');
    case 'forbidden':
      return t('errorForbidden');
    case 'unauthenticated':
      return t('errorUnauthenticated');
    case 'busy':
      return t('errorBusy');
    case 'csrf':
    case 'bad_origin':
    case 'missing_origin':
      return t('errorRefresh');
    case 'too_many_connections':
      return t('errorTooManyConnections');
    case 'network':
      return t('errorNetwork');
    case 'validation':
      return error.fields[0]?.message ?? error.message;
    case 'bad_request':
    case 'unsupported_media_type':
    case 'payload_too_large':
      return error.message;
    default:
      return t('errorUnexpected');
  }
}

/** Field-level problems keyed by field name, for forms. */
export function fieldErrors(error: unknown): Record<string, string> {
  if (!(error instanceof ApiError)) {
    return {};
  }
  return Object.fromEntries(error.fields.map((problem) => [problem.field, problem.message]));
}
