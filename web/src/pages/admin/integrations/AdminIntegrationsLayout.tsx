import { NavLink, Outlet } from 'react-router';
import { ti } from '../../../i18n/messages.integrations';
import type { IntegrationsMessageKey } from '../../../i18n/messages.integrations';
import { cx } from '../../../styles/cx';
import styles from './integrations.module.css';

const SECTIONS: { to: string; label: IntegrationsMessageKey }[] = [
  { to: '/admin/integrations/providers', label: 'navProviders' },
  { to: '/admin/integrations/search', label: 'navSearch' },
  { to: '/admin/integrations/github', label: 'navGitHub' },
  { to: '/admin/integrations/sandbox-policy', label: 'navSandbox' },
];

/** The integrations area: a labelled list of links to each section (not tabs), then the section. */
export function Component() {
  return (
    <>
      <nav
        aria-label={ti('integrationsNav')}
        className={styles.subnav}
        data-testid="integrations-nav"
      >
        <ul>
          {SECTIONS.map((section) => (
            <li key={section.to}>
              <NavLink
                to={section.to}
                className={({ isActive }) => cx(styles.subnavLink, isActive && styles.subnavActive)}
              >
                {ti(section.label)}
              </NavLink>
            </li>
          ))}
        </ul>
      </nav>
      <Outlet />
    </>
  );
}
