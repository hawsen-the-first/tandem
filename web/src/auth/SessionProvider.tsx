import { cx } from '../styles/cx';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { createContext, useCallback, useContext, useMemo } from 'react';
import type { ReactNode } from 'react';
import { useNavigate } from 'react-router';
import { call, client } from '../api/client';
import { setCsrfToken } from '../api/csrf';
import { ApiError } from '../api/errors';
import { ME_KEY } from '../api/keys';
import type { components } from '../api/schema';
import { t } from '../i18n/messages';
import shared from '../styles/shared.module.css';

export type Me = components['schemas']['MeResponse'];

interface SessionValue {
  me: Me | null;
  /** Re-read the session after sign-in, MFA enrolment or any change that affects it. */
  refresh: () => Promise<void>;
  signOut: () => Promise<void>;
}

const SessionContext = createContext<SessionValue | null>(null);

/**
 * The session comes only from the server (/api/auth/me): user, role and scope. The client never
 * decides authority; hiding a menu item is convenience and the server enforces access (BR-9).
 */
export function SessionProvider({ children }: { children: ReactNode }) {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const query = useQuery({
    queryKey: ME_KEY,
    retry: false,
    staleTime: 30_000,
    queryFn: async (): Promise<Me | null> => {
      try {
        const me = await call(client.GET('/api/auth/me'));
        setCsrfToken(me.csrf_token);
        return me;
      } catch (error) {
        if (error instanceof ApiError && error.status === 401) {
          setCsrfToken(null);
          return null;
        }
        throw error;
      }
    },
  });

  const refresh = useCallback(async () => {
    await queryClient.invalidateQueries({ queryKey: ME_KEY });
  }, [queryClient]);

  const signOut = useCallback(async () => {
    try {
      await call(client.POST('/api/auth/sign-out'));
    } finally {
      setCsrfToken(null);
      queryClient.clear();
      void navigate('/sign-in', { replace: true });
    }
  }, [navigate, queryClient]);

  const value = useMemo(
    () => ({ me: query.data ?? null, refresh, signOut }),
    [query.data, refresh, signOut],
  );

  if (query.isPending) {
    return (
      <p className={cx(shared.narrow, shared.muted)} role="status" data-testid="session-loading">
        {t('loading')}
      </p>
    );
  }
  if (query.isError) {
    return (
      <div className={cx(shared.card, shared.narrow)} role="alert" data-testid="session-error">
        <h1>{t('errorNetwork')}</h1>
        <button
          type="button"
          className={shared.button}
          onClick={() => {
            void query.refetch();
          }}
        >
          {t('retry')}
        </button>
      </div>
    );
  }
  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}

export function useSession(): SessionValue {
  const value = useContext(SessionContext);
  if (value === null) {
    throw new Error('useSession must be used inside SessionProvider');
  }
  return value;
}

export function isAdmin(me: Me | null): boolean {
  return me?.user.role === 'admin';
}

export function hasFullScope(me: Me | null): boolean {
  return me?.scope === 'full';
}
