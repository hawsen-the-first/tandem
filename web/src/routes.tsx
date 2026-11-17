import type { ComponentType } from 'react';
import { Navigate } from 'react-router';
import type { RouteObject } from 'react-router';
import { RequireAdmin, RequireAuth } from './auth/guards';
import { Root } from './Root';
import { AppShell } from './shell/AppShell';

type PageModule = { Component: ComponentType };

// Pages load on demand; each page module exports `Component`.
const page = (load: () => Promise<PageModule>): Pick<RouteObject, 'lazy'> => ({
  lazy: async () => ({ Component: (await load()).Component }),
});

export const appRoutes: RouteObject[] = [
  {
    element: <Root />,
    children: [
      { path: '/setup', ...page(() => import('./pages/SetupPage')) },
      { path: '/sign-in', ...page(() => import('./pages/SignInPage')) },
      { path: '/accept-invitation', ...page(() => import('./pages/AcceptInvitationPage')) },
      { path: '/reset-password', ...page(() => import('./pages/ResetPasswordPage')) },
      {
        element: <RequireAuth scope="any" />,
        children: [{ path: '/mfa/enrol', ...page(() => import('./pages/MfaEnrolmentPage')) }],
      },
      {
        element: <RequireAuth scope="full" />,
        children: [
          {
            element: <AppShell />,
            children: [
              { path: '/', ...page(() => import('./pages/HomePage')) },
              { path: '/chats', ...page(() => import('./pages/chat/ChatSessionListPage')) },
              { path: '/chats/:sessionId', ...page(() => import('./pages/chat/ChatSessionPage')) },
              { path: '/account', ...page(() => import('./pages/AccountPage')) },
              {
                element: <RequireAdmin />,
                children: [
                  { path: '/admin/users', ...page(() => import('./pages/admin/UsersPage')) },
                  { path: '/admin/settings', ...page(() => import('./pages/admin/SettingsPage')) },
                  { path: '/admin/audit', ...page(() => import('./pages/admin/AuditLogPage')) },
                  {
                    path: '/admin/integrations',
                    ...page(() => import('./pages/admin/integrations/AdminIntegrationsLayout')),
                    children: [
                      { index: true, element: <Navigate to="providers" replace /> },
                      {
                        path: 'providers',
                        ...page(() => import('./pages/admin/integrations/ProvidersPage')),
                      },
                      {
                        path: 'search',
                        ...page(() => import('./pages/admin/integrations/SearchPage')),
                      },
                      {
                        path: 'github',
                        ...page(() => import('./pages/admin/integrations/GitHubPage')),
                      },
                      {
                        path: 'sandbox-policy',
                        ...page(() => import('./pages/admin/integrations/SandboxPolicyPage')),
                      },
                    ],
                  },
                ],
              },
            ],
          },
        ],
      },
      { path: '*', ...page(() => import('./pages/NotFoundPage')) },
    ],
  },
];
