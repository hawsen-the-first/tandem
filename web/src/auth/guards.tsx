import { Navigate, Outlet, useLocation } from 'react-router';
import NotFoundPage from '../pages/NotFoundPage';
import { hasFullScope, isAdmin, useSession } from './SessionProvider';

interface RequireAuthProps {
  /** `any`: a signed-in person, even one who may only enrol MFA. `full`: a full-scope sign-in. */
  scope: 'any' | 'full';
}

/** Sends visitors who are not signed in to sign-in, and enrolment-only sign-ins to MFA enrolment. */
export function RequireAuth({ scope }: RequireAuthProps) {
  const { me } = useSession();
  const location = useLocation();
  if (me === null) {
    return <Navigate to="/sign-in" replace state={{ from: location.pathname }} />;
  }
  if (scope === 'full' && !hasFullScope(me)) {
    return <Navigate to="/mfa/enrol" replace />;
  }
  return <Outlet />;
}

/** Admin pages show "not found" to everyone else (BR-9.2); the server enforces this too. */
export function RequireAdmin() {
  const { me } = useSession();
  if (!isAdmin(me)) {
    return <NotFoundPage />;
  }
  return <Outlet />;
}
