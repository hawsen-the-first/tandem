import { QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';
import type { RenderResult } from '@testing-library/react';
import { RouterProvider, createMemoryRouter } from 'react-router';
import { createQueryClient } from '../api/queryClient';
import { setCsrfToken } from '../api/csrf';
import { ToastProvider } from '../components/Toast';
import { appRoutes } from '../routes';
import { ThemeProvider } from '../theme/ThemeProvider';

/** Render the real route table at `path`, with the real providers and whatever `mockApi` serves. */
export function renderApp(path: string): RenderResult {
  setCsrfToken(null);
  const router = createMemoryRouter(appRoutes, { initialEntries: [path] });
  return render(
    <ThemeProvider>
      <QueryClientProvider client={createQueryClient()}>
        <ToastProvider>
          <RouterProvider router={router} />
        </ToastProvider>
      </QueryClientProvider>
    </ThemeProvider>,
  );
}
