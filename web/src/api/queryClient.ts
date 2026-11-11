import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query';
import { ApiError } from './errors';
import { ME_KEY } from './keys';

/**
 * One client for the app. A 401 anywhere means the sign-in ended (expired, signed out elsewhere,
 * deactivated), so the session is re-read and the guards send the person to sign-in.
 */
export function createQueryClient(): QueryClient {
  const client: QueryClient = new QueryClient({
    defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
    queryCache: new QueryCache({
      onError: (error) => {
        onFailure(client, error);
      },
    }),
    mutationCache: new MutationCache({
      onError: (error) => {
        onFailure(client, error);
      },
    }),
  });
  return client;
}

function onFailure(client: QueryClient, error: unknown): void {
  if (error instanceof ApiError && error.status === 401 && error.code === 'unauthenticated') {
    void client.invalidateQueries({ queryKey: ME_KEY });
  }
}
