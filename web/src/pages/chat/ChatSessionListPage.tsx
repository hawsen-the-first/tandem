import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import type { SyntheticEvent } from 'react';
import { Link, useNavigate } from 'react-router';
import { call, client } from '../../api/client';
import { errorMessage, fieldErrors } from '../../api/messages';
import { FormField } from '../../components/FormField';
import { ResponsiveTable } from '../../components/ResponsiveTable';
import { t } from '../../i18n/messages';
import { TITLE_MAX, c } from '../../i18n/messages.chat';
import shared from '../../styles/shared.module.css';
import { cx } from '../../styles/cx';
import { SESSIONS_KEY, fetchSessions } from './chatQueries';
import type { ChatSession } from './chatQueries';

export function validateTitle(title: string): string | undefined {
  const length = Array.from(title.trim()).length;
  if (length === 0) {
    return c('titleRequired');
  }
  return length > TITLE_MAX ? c('titleTooLong', { max: TITLE_MAX }) : undefined;
}

export function Component() {
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const sessions = useQuery({ queryKey: SESSIONS_KEY, queryFn: fetchSessions });
  const [title, setTitle] = useState('');
  const [problem, setProblem] = useState<string | undefined>(undefined);
  const [failure, setFailure] = useState<string | undefined>(undefined);

  const create = useMutation({
    mutationFn: (text: string) =>
      call(client.POST('/api/chat/sessions', { body: { title: text.trim() } })),
    onSuccess: async (session) => {
      await queryClient.invalidateQueries({ queryKey: SESSIONS_KEY });
      void navigate(`/chats/${session.id}`);
    },
    onError: (error) => {
      setProblem(fieldErrors(error)['title']);
      setFailure(errorMessage(error));
    },
  });

  const submit = (event: SyntheticEvent) => {
    event.preventDefault();
    const found = validateTitle(title);
    setProblem(found);
    setFailure(undefined);
    if (found === undefined) {
      create.mutate(title);
    }
  };

  return (
    <div data-testid="chat-list-page">
      <h1>{c('listTitle')}</h1>
      <form className={cx(shared.card, shared.stack)} onSubmit={submit} noValidate>
        <h2>{c('listCreateTitle')}</h2>
        <FormField
          label={c('listTitleLabel')}
          hint={c('listTitleHint', { max: TITLE_MAX })}
          error={problem}
          required
          testId="chat-title"
        >
          {(control) => (
            <input
              {...control}
              value={title}
              autoComplete="off"
              data-testid="chat-title-input"
              onChange={(event) => {
                setTitle(event.target.value);
              }}
            />
          )}
        </FormField>
        {failure !== undefined && problem === undefined ? (
          <p
            className={cx(shared.banner, shared.bannerError)}
            role="alert"
            data-testid="chat-create-error"
          >
            {failure}
          </p>
        ) : null}
        <div>
          <button
            type="submit"
            className={shared.button}
            disabled={create.isPending}
            data-testid="chat-create"
          >
            {create.isPending ? c('listCreating') : c('listCreate')}
          </button>
        </div>
      </form>

      <h2>{c('listCaption')}</h2>
      {sessions.isPending ? (
        <p role="status" className={shared.muted} data-testid="chat-list-loading">
          {t('loading')}
        </p>
      ) : sessions.isError ? (
        <p
          className={cx(shared.banner, shared.bannerError)}
          role="alert"
          data-testid="chat-list-error"
        >
          {c('listLoadError')} {errorMessage(sessions.error)}{' '}
          <button
            type="button"
            className={shared.linkButton}
            onClick={() => {
              void sessions.refetch();
            }}
          >
            {t('retry')}
          </button>
        </p>
      ) : (
        <ResponsiveTable<ChatSession>
          caption={c('listCaption')}
          rows={sessions.data}
          rowKey={(session) => session.id}
          emptyState={c('listEmpty')}
          testId="chat-list"
          columns={[
            {
              key: 'title',
              header: c('listColumnTitle'),
              render: (session) => (
                <Link to={`/chats/${session.id}`} data-testid="chat-list-link">
                  {session.title}
                </Link>
              ),
            },
            {
              key: 'created',
              header: c('listColumnCreated'),
              render: (session) => (
                <time dateTime={session.created_at}>
                  {new Date(session.created_at).toLocaleString()}
                </time>
              ),
            },
          ]}
        />
      )}
    </div>
  );
}
