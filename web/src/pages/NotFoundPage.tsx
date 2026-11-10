import { cx } from '../styles/cx';
import { Link } from 'react-router';
import { t } from '../i18n/messages';
import shared from '../styles/shared.module.css';

export default function NotFoundPage() {
  return (
    <div className={cx(shared.card, shared.narrow)} data-testid="not-found-page">
      <h1>{t('pageNotFoundTitle')}</h1>
      <p>{t('pageNotFoundBody')}</p>
      <Link to="/" className={shared.button}>
        {t('goHome')}
      </Link>
    </div>
  );
}

export { NotFoundPage as Component };
