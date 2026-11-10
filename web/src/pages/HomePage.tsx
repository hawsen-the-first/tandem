import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { useSession } from '../auth/SessionProvider';
import { t } from '../i18n/messages';
import { c } from '../i18n/messages.chat';
import shared from '../styles/shared.module.css';
import { cx } from '../styles/cx';
import { SESSIONS_KEY, fetchSessions } from './chat/chatQueries';

const RECENT = 5;

export function Component() {
  const { me } = useSession();
  const sessions = useQuery({ queryKey: SESSIONS_KEY, queryFn: fetchSessions });
  const recent = sessions.data?.slice(0, RECENT) ?? [];
  return (
    <div className={shared.stack} data-testid="home-page">
      <h1>{c('homeTitle')}</h1>
      {me?.mfa.remind_regenerate === true ? (
        <section
          className={cx(shared.banner, shared.bannerWarning)}
          data-testid="home-recovery-reminder"
        >
          <h2>{c('homeReminderTitle')}</h2>
          <p>{c('homeReminderBody')}</p>
          <Link to="/account">{c('homeReminderLink')}</Link>
        </section>
      ) : null}
      <section className={shared.card} aria-labelledby="home-recent-heading">
        <h2 id="home-recent-heading">{c('homeRecentTitle')}</h2>
        {sessions.isPending ? (
          <p role="status" className={shared.muted}>
            {t('loading')}
          </p>
        ) : recent.length === 0 ? (
          <p data-testid="home-no-chats">{c('homeNoChats')}</p>
        ) : (
          <ul data-testid="home-recent-list">
            {recent.map((session) => (
              <li key={session.id}>
                <Link to={`/chats/${session.id}`}>{session.title}</Link>
              </li>
            ))}
          </ul>
        )}
        <div className={shared.row}>
          <Link to="/chats" className={shared.button} data-testid="home-new-chat">
            {c('homeNewChat')}
          </Link>
          <Link to="/chats">{c('homeViewAll')}</Link>
        </div>
      </section>
    </div>
  );
}
