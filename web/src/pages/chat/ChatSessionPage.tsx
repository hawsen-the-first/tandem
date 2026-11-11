import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { KeyboardEvent, SyntheticEvent } from 'react';
import { Link, useParams } from 'react-router';
import { call, client } from '../../api/client';
import { errorMessage } from '../../api/messages';
import { FormField } from '../../components/FormField';
import { t } from '../../i18n/messages';
import { BODY_MAX, c } from '../../i18n/messages.chat';
import type { ClientEvent, ConnectionState } from '../../realtime/eventsClient';
import { useEvents } from '../../realtime/useEvents';
import shared from '../../styles/shared.module.css';
import { cx } from '../../styles/cx';
import styles from './ChatSessionPage.module.css';
import {
  SESSIONS_KEY,
  appendUnique,
  fetchMessages,
  fetchSessions,
  messagesKey,
} from './chatQueries';
import type { ChatMessage } from './chatQueries';

const NEAR_LIMIT = 200;

export function validateBody(body: string): string | undefined {
  const length = Array.from(body.trim()).length;
  if (length === 0) {
    return c('composerRequired');
  }
  return length > BODY_MAX ? c('composerTooLong', { max: BODY_MAX }) : undefined;
}

function connectionText(state: ConnectionState): string {
  switch (state) {
    case 'live':
      return c('connectionLive');
    case 'reconnecting':
      return c('connectionReconnecting');
    case 'stopped':
    case 'connecting':
      return c('connectionConnecting');
  }
}

function MessageItem({ message }: { message: ChatMessage }) {
  const agent = message.author_kind === 'agent';
  return (
    <li
      className={cx(styles.message, agent ? styles.agent : styles.person)}
      data-testid="chat-message"
      data-author-kind={message.author_kind}
    >
      <p className={styles.author} data-testid="chat-message-author">
        {agent ? message.author : c('authorYou')}{' '}
        <time dateTime={message.created_at} className={shared.muted}>
          {new Date(message.created_at).toLocaleTimeString()}
        </time>
      </p>
      <p className={styles.body}>{message.body}</p>
    </li>
  );
}

export function Component() {
  const { sessionId = '' } = useParams();
  const queryClient = useQueryClient();
  const sessions = useQuery({ queryKey: SESSIONS_KEY, queryFn: fetchSessions });
  const messages = useQuery({
    queryKey: messagesKey(sessionId),
    queryFn: () => fetchMessages(sessionId),
    enabled: sessionId !== '',
  });
  const [draft, setDraft] = useState('');
  const [problem, setProblem] = useState<string | undefined>(undefined);
  const [failure, setFailure] = useState<string | undefined>(undefined);
  const end = useRef<HTMLDivElement>(null);

  const reload = useCallback(() => {
    void queryClient.invalidateQueries({ queryKey: messagesKey(sessionId) });
  }, [queryClient, sessionId]);

  const onEvent = useCallback(
    (event: ClientEvent) => {
      if (event.type === 'event') {
        if (event.event.chatSessionId === sessionId) {
          reload();
        }
      } else if (event.type === 'resync' || event.type === 'reconnected') {
        reload();
      }
    },
    [reload, sessionId],
  );
  const connection = useEvents(onEvent);

  const send = useMutation({
    mutationFn: (body: string) =>
      call(
        client.POST('/api/chat/sessions/{id}/messages', {
          params: { path: { id: sessionId } },
          body: { body },
        }),
      ),
    onSuccess: (message) => {
      queryClient.setQueryData<ChatMessage[]>(messagesKey(sessionId), (current) =>
        appendUnique(current, message),
      );
      setDraft('');
      reload();
    },
    onError: (error) => {
      setFailure(errorMessage(error));
    },
  });

  const count = messages.data?.length ?? 0;
  useEffect(() => {
    end.current?.scrollIntoView({ block: 'end' });
  }, [count]);

  const submit = (event?: SyntheticEvent) => {
    event?.preventDefault();
    if (send.isPending) {
      return;
    }
    const found = validateBody(draft);
    setProblem(found);
    setFailure(undefined);
    if (found === undefined) {
      send.mutate(draft);
    }
  };

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      submit();
    }
  };

  const title = sessions.data?.find((session) => session.id === sessionId)?.title;
  const remaining = BODY_MAX - Array.from(draft).length;
  const nearLimit = remaining <= NEAR_LIMIT;
  const lastIsFromPerson = messages.data?.at(-1)?.author_kind === 'user';

  return (
    <div data-testid="chat-session-page">
      <p>
        <Link to="/chats">{c('sessionBack')}</Link>
      </p>
      <h1>{title ?? c('sessionLog')}</h1>
      <p
        className={styles.connection}
        role="status"
        aria-live="polite"
        data-testid="chat-connection"
        data-state={connection}
      >
        {connectionText(connection)}
      </p>

      {messages.isPending ? (
        <p role="status" className={shared.muted} data-testid="chat-messages-loading">
          {t('loading')}
        </p>
      ) : messages.isError ? (
        <p
          className={cx(shared.banner, shared.bannerError)}
          role="alert"
          data-testid="chat-messages-error"
        >
          {c('sessionLoadError')} {errorMessage(messages.error)}{' '}
          <button
            type="button"
            className={shared.linkButton}
            onClick={() => {
              void messages.refetch();
            }}
          >
            {t('retry')}
          </button>
        </p>
      ) : (
        <div
          className={styles.log}
          role="log"
          aria-live="polite"
          aria-relevant="additions"
          aria-label={c('sessionLog')}
          tabIndex={0}
          data-testid="chat-log"
        >
          {messages.data.length === 0 ? (
            <p className={shared.muted} data-testid="chat-empty">
              {c('sessionEmpty')}
            </p>
          ) : (
            <ol className={styles.list}>
              {messages.data.map((message) => (
                <MessageItem key={message.id} message={message} />
              ))}
            </ol>
          )}
          {lastIsFromPerson ? (
            <p className={shared.muted} data-testid="chat-waiting">
              {c('agentWaiting')}
            </p>
          ) : null}
          <div ref={end} />
        </div>
      )}

      <form className={cx(shared.stack, styles.composer)} onSubmit={submit} noValidate>
        <FormField
          label={c('composerLabel')}
          hint={c('composerHint')}
          error={problem}
          testId="chat-composer"
        >
          {(control) => (
            <textarea
              {...control}
              rows={3}
              value={draft}
              disabled={send.isPending}
              data-testid="chat-composer-input"
              onChange={(event) => {
                setDraft(event.target.value);
              }}
              onKeyDown={onKeyDown}
            />
          )}
        </FormField>
        {nearLimit ? (
          <p
            role="status"
            className={remaining < 0 ? styles.over : shared.muted}
            data-testid="chat-counter"
          >
            {c('composerRemaining', { count: remaining })}
          </p>
        ) : null}
        {failure !== undefined ? (
          <p
            className={cx(shared.banner, shared.bannerError)}
            role="alert"
            data-testid="chat-send-error"
          >
            {failure}
          </p>
        ) : null}
        <div>
          <button
            type="submit"
            className={shared.button}
            disabled={send.isPending}
            data-testid="chat-send"
          >
            {send.isPending ? c('composerSending') : c('composerSend')}
          </button>
        </div>
      </form>
    </div>
  );
}
