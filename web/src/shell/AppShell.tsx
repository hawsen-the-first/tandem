import { useEffect, useRef, useState } from 'react';
import { NavLink, Outlet, useLocation } from 'react-router';
import { isAdmin, useSession } from '../auth/SessionProvider';
import { cx } from '../styles/cx';
import { t } from '../i18n/messages';
import { ThemeToggle } from '../theme/ThemeToggle';
import styles from './AppShell.module.css';

export function AppShell() {
  const { me, signOut } = useSession();
  const location = useLocation();
  const [openPath, setOpenPath] = useState<string | null>(null);
  const menuOpen = openPath === location.pathname;
  const main = useRef<HTMLElement>(null);
  const firstRender = useRef(true);

  // After navigating, move focus to the page content so keyboard and screen-reader users start at
  // the top of the new page; not on the first load.
  useEffect(() => {
    if (firstRender.current) {
      firstRender.current = false;
      return;
    }
    main.current?.focus({ preventScroll: true });
  }, [location.pathname]);

  const link = ({ isActive }: { isActive: boolean }) => cx(styles.link, isActive && styles.active);

  return (
    <div className={styles.shell}>
      <a href="#main" className={styles.skip}>
        {t('skipToContent')}
      </a>
      <header className={styles.header}>
        <NavLink to="/" className={cx(styles.brand)} data-testid="brand">
          {t('appName')}
        </NavLink>
        <button
          type="button"
          className={styles.menuButton}
          aria-expanded={menuOpen}
          aria-controls="primary-nav"
          data-testid="menu-button"
          onClick={() => {
            setOpenPath(menuOpen ? null : location.pathname);
          }}
        >
          {t('navMenu')}
        </button>
        <nav id="primary-nav" aria-label={t('navPrimary')} className={`XX`}>
          <NavLink to="/chats" className={link}>
            {t('navChats')}
          </NavLink>
          <NavLink to="/account" className={link}>
            {t('navAccount')}
          </NavLink>
          {isAdmin(me) ? (
            <>
              <NavLink to="/admin/users" className={link}>
                {t('navUsers')}
              </NavLink>
              <NavLink to="/admin/settings" className={link}>
                {t('navSettings')}
              </NavLink>
              <NavLink to="/admin/audit" className={link}>
                {t('navAudit')}
              </NavLink>
            </>
          ) : null}
        </nav>
        <div className={styles.tools}>
          <ThemeToggle />
          <details className={styles.userMenu} data-testid="user-menu">
            <summary>
              {me?.user.display_name}
              <span className="visually-hidden"> {t('userMenu')}</span>
            </summary>
            <div className={styles.userMenuBody}>
              <p className={styles.userEmail}>{me?.user.email}</p>
              <button
                type="button"
                className={styles.signOut}
                data-testid="sign-out"
                onClick={() => {
                  void signOut();
                }}
              >
                {t('signOut')}
              </button>
            </div>
          </details>
        </div>
      </header>
      <main id="main" ref={main} tabIndex={-1} className={styles.main}>
        <Outlet />
      </main>
    </div>
  );
}
