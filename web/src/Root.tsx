import { Outlet } from 'react-router';
import { SessionProvider } from './auth/SessionProvider';

/** The root route: every page below it can read the session. */
export function Root() {
  return (
    <SessionProvider>
      <Outlet />
    </SessionProvider>
  );
}
