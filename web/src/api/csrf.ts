// The CSRF token is held in memory only. The session provider sets it from /api/auth/me and clears
// it on sign-out; the API client sends it on every state-changing request.
let token: string | null = null;

export function setCsrfToken(value: string | null): void {
  token = value;
}

export function getCsrfToken(): string | null {
  return token;
}
