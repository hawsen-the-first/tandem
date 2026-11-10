import { QueryClientProvider } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { RouterProvider } from 'react-router';
import type { DataRouter } from 'react-router';
import { ErrorBoundary } from './components/ErrorBoundary';
import { ToastProvider } from './components/Toast';
import { ThemeProvider } from './theme/ThemeProvider';

interface AppProps {
  router: DataRouter;
  queryClient: QueryClient;
}

/** Providers in order: errors, theme, server state, toasts, then the router (which supplies the session). */
export function App({ router, queryClient }: AppProps) {
  return (
    <ErrorBoundary>
      <ThemeProvider>
        <QueryClientProvider client={queryClient}>
          <ToastProvider>
            <RouterProvider router={router} />
          </ToastProvider>
        </QueryClientProvider>
      </ThemeProvider>
    </ErrorBoundary>
  );
}
