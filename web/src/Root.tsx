import { Outlet } from 'react-router';
import { SessionProvider } from './auth/SessionProvider';
import { SiteFooter } from './components/SiteFooter';

/** The root route: every page below it can read the session; every page ends with the legal notices. */
export function Root() {
  return (
    <>
      <SessionProvider>
        <Outlet />
      </SessionProvider>
      <SiteFooter />
    </>
  );
}
