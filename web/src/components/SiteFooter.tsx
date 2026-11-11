import { useQuery } from '@tanstack/react-query';
import { call, client } from '../api/client';
import { t } from '../i18n/messages';
import styles from './SiteFooter.module.css';

const FALLBACK_COPYRIGHT = 'Copyright (C) 2026 Reuben Hawthorne-Jensen and Tandem contributors';
const FALLBACK_LICENSE_URL = 'https://www.gnu.org/licenses/agpl-3.0.html';
const FALLBACK_UPSTREAM_URL = 'https://github.com/hawsen-the-first/tandem';

/**
 * The legal notices of the interface: copyright, no warranty, the licence and how to read it, the
 * source of this running version (AGPL section 13) and the "Based on Tandem" credit that the
 * additional terms require forks to keep. The text is shown even if the details cannot be loaded.
 */
export function SiteFooter() {
  const about = useQuery({
    queryKey: ['about'],
    queryFn: () => call(client.GET('/api/about')),
    staleTime: Infinity,
  });
  const details = about.data;
  return (
    <footer className={styles.footer} aria-label={t('footerLabel')} data-testid="site-footer">
      <p>
        {t('appName')} {details?.version ?? ''} · {details?.copyright ?? FALLBACK_COPYRIGHT}
      </p>
      <p>
        {t('footerWarranty')}{' '}
        <a href={details?.license_url ?? FALLBACK_LICENSE_URL} data-testid="footer-license">
          {t('footerViewLicense')}
        </a>
        .
      </p>
      <p>
        {details === undefined ? null : (
          <>
            <a href={details.source_url} data-testid="footer-source">
              {t('footerSource')}
            </a>
            {' · '}
          </>
        )}
        {t('footerBasedOn')}{' '}
        <a href={details?.upstream_url ?? FALLBACK_UPSTREAM_URL} data-testid="footer-upstream">
          {t('appName')}
        </a>
      </p>
    </footer>
  );
}
