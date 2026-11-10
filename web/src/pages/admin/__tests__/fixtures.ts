import { ME_ADMIN } from '../../../test/mockApi';
import type { MockApi, MockHandler } from '../../../test/mockApi';
import { mockApi } from '../../../test/mockApi';

export const ADA = {
  id: ME_ADMIN.user.id,
  email: 'ada@example.test',
  display_name: 'Ada Admin',
  role: 'admin',
  status: 'active',
  created_at: '2026-01-01T00:00:00Z',
  last_sign_in_at: '2026-03-02T09:30:00Z',
  mfa_enrolled: true,
};
export const BO = {
  id: '019b0000-0000-7000-8000-0000000000b0',
  email: 'bo@example.test',
  display_name: 'Bo Builder',
  role: 'admin',
  status: 'active',
  created_at: '2026-01-02T00:00:00Z',
  last_sign_in_at: null,
  mfa_enrolled: true,
};
export const MIA = {
  id: '019b0000-0000-7000-8000-0000000000c0',
  email: 'mia@example.test',
  display_name: 'Mia Member',
  role: 'member',
  status: 'active',
  created_at: '2026-01-03T00:00:00Z',
  last_sign_in_at: '2026-03-01T08:00:00Z',
  mfa_enrolled: false,
};
export const DEE = {
  id: '019b0000-0000-7000-8000-0000000000d0',
  email: 'dee@example.test',
  display_name: 'Dee Dormant',
  role: 'member',
  status: 'deactivated',
  created_at: '2026-01-04T00:00:00Z',
  last_sign_in_at: null,
  mfa_enrolled: false,
};
export const PENDING = {
  id: '019b0000-0000-7000-8000-0000000000e0',
  email: 'new@example.test',
  role: 'member',
  status: 'pending',
  created_at: '2026-03-01T00:00:00Z',
  expires_at: '2026-03-08T00:00:00Z',
};
export const ACCEPTED = {
  ...PENDING,
  id: '019b0000-0000-7000-8000-0000000000e1',
  email: 'old@example.test',
  status: 'accepted',
};

export const LAST_ADMIN_ERROR = {
  status: 422,
  body: {
    error: {
      code: 'validation',
      message: 'Check the highlighted fields.',
      fields: [{ field: 'user', message: 'At least one active admin is required' }],
    },
  },
};

/** An admin signed in, with the users and invitations endpoints served. */
export function adminApi(
  extra: Record<string, MockHandler> = {},
  users = [ADA, BO, MIA, DEE],
): MockApi {
  return mockApi({
    'GET /api/auth/me': { body: ME_ADMIN },
    'GET /api/admin/users': { body: users },
    'GET /api/admin/invitations': { body: [PENDING, ACCEPTED] },
    ...extra,
  });
}
