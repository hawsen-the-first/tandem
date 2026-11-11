import type { components } from './schema';

type ErrorEnvelope = components['schemas']['ErrorEnvelope'];

export interface FieldProblem {
  field: string;
  message: string;
}

/** A failed API call, with the stable code the server sent and, for throttling, when to retry. */
export class ApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly fields: FieldProblem[];
  readonly retryAfterSeconds: number | null;

  constructor(init: {
    status: number;
    code: string;
    message: string;
    fields?: FieldProblem[];
    retryAfterSeconds?: number | null;
  }) {
    super(init.message);
    this.name = 'ApiError';
    this.status = init.status;
    this.code = init.code;
    this.fields = init.fields ?? [];
    this.retryAfterSeconds = init.retryAfterSeconds ?? null;
  }
}

function isEnvelope(value: unknown): value is ErrorEnvelope {
  if (typeof value !== 'object' || value === null || !('error' in value)) {
    return false;
  }
  const { error } = value;
  return typeof error === 'object' && error !== null && 'code' in error && 'message' in error;
}

/** Build an ApiError from a response whose body has already been parsed (or failed to parse). */
export function toApiError(status: number, body: unknown, headers: Headers): ApiError {
  const retry = Number(headers.get('retry-after'));
  const retryAfterSeconds = Number.isFinite(retry) && retry > 0 ? retry : null;
  if (isEnvelope(body)) {
    return new ApiError({
      status,
      code: body.error.code,
      message: body.error.message,
      fields: body.error.fields ?? [],
      retryAfterSeconds,
    });
  }
  return new ApiError({
    status,
    code: status >= 500 ? 'internal' : 'unknown',
    message: 'Something went wrong. Nothing was changed.',
    retryAfterSeconds,
  });
}
