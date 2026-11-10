import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { axe } from 'vitest-axe';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeWebSocket, eventFrame, readyFrame } from '../../../realtime/__tests__/fakeWebSocket';
import { ME_MEMBER, mockApi } from '../../../test/mockApi';
import type { MockHandler } from '../../../test/mockApi';

interface FixtureMessage {
  id: string;
  chat_session_id: string;
  author_kind: string;
  author: string;
  body: string;
  created_at: string;
  in_reply_to: string | null;
}
import { renderApp } from '../../../test/render';

const session = { id: 's1', title: 'Planning', created_at: '2026-01-01T09:00:00Z' };
const userMessage: FixtureMessage = {
  id: 'm1',
  chat_session_id: 's1',
  author_kind: 'user',
  author: ME_MEMBER.user.id,
  body: 'hello team',
  created_at: '2026-01-01T09:01:00Z',
  in_reply_to: null,
};
const agentMessage: FixtureMessage = {
  id: 'm2',
  chat_session_id: 's1',
  author_kind: 'agent',
  author: 'mock-agent',
  body: 'Received: hello team',
  created_at: '2026-01-01T09:01:01Z',
  in_reply_to: 'm1',
};

beforeEach(() => {
  FakeWebSocket.reset();
  vi.stubGlobal('WebSocket', FakeWebSocket);
  Element.prototype.scrollIntoView = vi.fn();
});

describe('ChatSessionListPage', () => {
  it('lists the person’s chats as links', async () => {
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [session] },
    });
    const { container } = renderApp('/chats');
    const link = await screen.findByTestId('chat-list-link');
    expect(link).toHaveTextContent('Planning');
    expect(link).toHaveAttribute('href', '/chats/s1');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows an empty state and a loading state', async () => {
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [] },
    });
    renderApp('/chats');
    expect(await screen.findByTestId('chat-list-empty')).toHaveTextContent('no chats yet');
  });

  it('shows an error with a retry that works', async () => {
    const user = userEvent.setup();
    let fail = true;
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': () =>
        fail
          ? { status: 500, body: { error: { code: 'internal', message: 'x' } } }
          : { body: [session] },
    });
    renderApp('/chats');
    const error = await screen.findByTestId('chat-list-error');
    expect(error).toHaveTextContent('could not be loaded');
    fail = false;
    await user.click(within(error).getByRole('button', { name: 'Try again' }));
    expect(await screen.findByTestId('chat-list-link')).toBeInTheDocument();
  });

  it('refuses an empty or too-long title without calling the server', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [] },
    });
    renderApp('/chats');
    await screen.findByTestId('chat-list-empty');
    await user.click(screen.getByTestId('chat-create'));
    expect(screen.getByTestId('chat-title-error')).toHaveTextContent('Enter a title');
    expect(screen.getByTestId('chat-title-input')).toHaveAttribute('aria-invalid', 'true');
    fireEvent.change(screen.getByTestId('chat-title-input'), {
      target: { value: 'x'.repeat(121) },
    });
    await user.click(screen.getByTestId('chat-create'));
    expect(screen.getByTestId('chat-title-error')).toHaveTextContent('up to 120 characters');
    expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
  });

  it('creates a chat with the trimmed title and opens it', async () => {
    const user = userEvent.setup();
    const api = mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [] },
      'POST /api/chat/sessions': {
        status: 201,
        body: { id: 's9', title: 'Retro', created_at: '2026-01-02T00:00:00Z' },
      },
      'GET /api/chat/sessions/s9/messages': { body: [] },
    });
    renderApp('/chats');
    await screen.findByTestId('chat-list-empty');
    await user.type(screen.getByTestId('chat-title-input'), '  Retro  {Enter}');
    expect(await screen.findByTestId('chat-session-page')).toBeInTheDocument();
    const post = api.calls.find(
      (call) => call.method === 'POST' && call.path === '/api/chat/sessions',
    );
    expect(post?.body).toEqual({ title: 'Retro' });
    expect(post?.headers.get('x-csrf-token')).toBe('csrf-member');
  });

  it('shows a server failure and keeps the typed title', async () => {
    const user = userEvent.setup();
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [] },
      'POST /api/chat/sessions': {
        status: 500,
        body: { error: { code: 'internal', message: 'x' } },
      },
    });
    renderApp('/chats');
    await screen.findByTestId('chat-list-empty');
    await user.type(screen.getByTestId('chat-title-input'), 'Retro');
    await user.click(screen.getByTestId('chat-create'));
    expect(await screen.findByTestId('chat-create-error')).toHaveTextContent('Nothing was changed');
    expect(screen.getByTestId('chat-title-input')).toHaveValue('Retro');
  });
});

function openSession(extra: Record<string, MockHandler> = {}) {
  const messages: FixtureMessage[] = [userMessage];
  const api = mockApi({
    'GET /api/auth/me': { body: ME_MEMBER },
    'GET /api/chat/sessions': { body: [session] },
    'GET /api/chat/sessions/s1/messages': () => ({ body: [...messages] }),
    ...extra,
  });
  return { api, messages };
}

describe('ChatSessionPage', () => {
  it('shows messages oldest first, labelled by author, in a log region', async () => {
    const { messages } = openSession();
    messages.push(agentMessage);
    const { container } = renderApp('/chats/s1');
    const log = await screen.findByRole('log', { name: 'Messages' });
    const items = within(log).getAllByTestId('chat-message');
    expect(items).toHaveLength(2);
    expect(within(items[0] as HTMLElement).getByTestId('chat-message-author')).toHaveTextContent(
      'You',
    );
    expect(within(items[1] as HTMLElement).getByTestId('chat-message-author')).toHaveTextContent(
      'mock-agent',
    );
    expect(screen.getByRole('heading', { name: 'Planning' })).toBeInTheDocument();
    expect(log).toHaveAttribute('aria-live', 'polite');
    expect(await axe(container)).toHaveNoViolations();
  });

  it('shows an empty state', async () => {
    const { messages } = openSession();
    messages.length = 0;
    renderApp('/chats/s1');
    expect(await screen.findByTestId('chat-empty')).toBeInTheDocument();
  });

  it('shows that a reply is awaited when the last message is the person’s own', async () => {
    openSession();
    renderApp('/chats/s1');
    expect(await screen.findByTestId('chat-waiting')).toBeInTheDocument();
  });

  it('shows an error with a retry when the chat cannot be loaded', async () => {
    const user = userEvent.setup();
    let fail = true;
    mockApi({
      'GET /api/auth/me': { body: ME_MEMBER },
      'GET /api/chat/sessions': { body: [session] },
      'GET /api/chat/sessions/s1/messages': () =>
        fail
          ? { status: 404, body: { error: { code: 'not_found', message: 'x' } } }
          : { body: [userMessage] },
    });
    const { container } = renderApp('/chats/s1');
    const error = await screen.findByTestId('chat-messages-error');
    expect(error).toHaveTextContent('Not found');
    expect(await axe(container)).toHaveNoViolations();
    fail = false;
    await user.click(within(error).getByRole('button', { name: 'Try again' }));
    expect(await screen.findByTestId('chat-log')).toBeInTheDocument();
  });

  it('posts with the button, clears the box and reconciles with the server list', async () => {
    const user = userEvent.setup();
    const { api, messages } = openSession({
      'POST /api/chat/sessions/s1/messages': () => {
        messages.push({ ...userMessage, id: 'm3', body: 'second' });
        return { status: 201, body: { ...userMessage, id: 'm3', body: 'second' } };
      },
    });
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await user.type(screen.getByTestId('chat-composer-input'), 'second');
    await user.click(screen.getByTestId('chat-send'));
    await waitFor(() => {
      expect(screen.getAllByTestId('chat-message')).toHaveLength(2);
    });
    expect(screen.getAllByText('second')).toHaveLength(1);
    expect(screen.getByTestId('chat-composer-input')).toHaveValue('');
    const post = api.calls.find((call) => call.method === 'POST');
    expect(post?.body).toEqual({ body: 'second' });
    expect(post?.headers.get('x-csrf-token')).toBe('csrf-member');
  });

  it('sends on Enter and adds a line on Shift+Enter', async () => {
    const user = userEvent.setup();
    const { api } = openSession({
      'POST /api/chat/sessions/s1/messages': { status: 201, body: { ...userMessage, id: 'm4' } },
    });
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    const input = screen.getByTestId('chat-composer-input');
    await user.type(input, 'line one{Shift>}{Enter}{/Shift}line two');
    expect(input).toHaveValue('line one\nline two');
    expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
    await user.keyboard('{Enter}');
    await waitFor(() => {
      expect(api.calls.some((call) => call.method === 'POST')).toBe(true);
    });
    expect(api.calls.find((call) => call.method === 'POST')?.body).toEqual({
      body: 'line one\nline two',
    });
  });

  it('refuses an empty or too-long message without calling the server', async () => {
    const user = userEvent.setup();
    const { api } = openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await user.click(screen.getByTestId('chat-send'));
    expect(screen.getByTestId('chat-composer-error')).toHaveTextContent('Write a message first');
    fireEvent.change(screen.getByTestId('chat-composer-input'), {
      target: { value: 'x'.repeat(4001) },
    });
    await user.click(screen.getByTestId('chat-send'));
    expect(screen.getByTestId('chat-composer-error')).toHaveTextContent('up to 4000 characters');
    expect(api.calls.some((call) => call.method === 'POST')).toBe(false);
  });

  it('shows the character counter only near the limit', async () => {
    openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    const input = screen.getByTestId('chat-composer-input');
    fireEvent.change(input, { target: { value: 'x'.repeat(100) } });
    expect(screen.queryByTestId('chat-counter')).not.toBeInTheDocument();
    fireEvent.change(input, { target: { value: 'x'.repeat(3900) } });
    expect(screen.getByTestId('chat-counter')).toHaveTextContent('100 characters left');
    expect(screen.getByTestId('chat-counter')).toHaveAttribute('role', 'status');
    fireEvent.change(input, { target: { value: 'x'.repeat(4003) } });
    expect(screen.getByTestId('chat-counter')).toHaveTextContent('-3 characters left');
  });

  it('keeps the text and shows the error when sending fails', async () => {
    const user = userEvent.setup();
    openSession({
      'POST /api/chat/sessions/s1/messages': {
        status: 500,
        body: { error: { code: 'internal', message: 'x' } },
      },
    });
    const { container } = renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await user.type(screen.getByTestId('chat-composer-input'), 'do not lose me');
    await user.click(screen.getByTestId('chat-send'));
    expect(await screen.findByTestId('chat-send-error')).toHaveTextContent('Nothing was changed');
    expect(screen.getByTestId('chat-composer-input')).toHaveValue('do not lose me');
    expect(screen.getByTestId('chat-send')).toBeEnabled();
    expect(await axe(container)).toHaveNoViolations();
  });

  it('disables the composer while a message is being sent', async () => {
    const user = userEvent.setup();
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    openSession({
      'POST /api/chat/sessions/s1/messages': async () => {
        await gate;
        return { status: 201, body: { ...userMessage, id: 'm5' } };
      },
    });
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await user.type(screen.getByTestId('chat-composer-input'), 'hi');
    await user.click(screen.getByTestId('chat-send'));
    expect(screen.getByTestId('chat-send')).toBeDisabled();
    expect(screen.getByTestId('chat-send')).toHaveTextContent('Sending…');
    expect(screen.getByTestId('chat-composer-input')).toBeDisabled();
    release();
    await waitFor(() => {
      expect(screen.getByTestId('chat-send')).toBeEnabled();
    });
  });

  it('shows the agent’s reply as it arrives over the live connection', async () => {
    const { messages } = openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await waitFor(() => {
      expect(FakeWebSocket.instances.length).toBeGreaterThan(0);
    });
    expect(screen.getByTestId('chat-connection')).toHaveTextContent('Connecting…');
    FakeWebSocket.latest.open();
    FakeWebSocket.latest.receive(readyFrame(10));
    await waitFor(() => {
      expect(screen.getByTestId('chat-connection')).toHaveTextContent('Live');
    });
    expect(screen.queryByText('Received: hello team')).not.toBeInTheDocument();
    messages.push(agentMessage);
    FakeWebSocket.latest.receive(eventFrame(11, 's1', true));
    expect(await screen.findByText('Received: hello team')).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByTestId('chat-waiting')).not.toBeInTheDocument();
    });
  });

  it('ignores events for other chats', async () => {
    const { api } = openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await waitFor(() => {
      expect(FakeWebSocket.instances.length).toBeGreaterThan(0);
    });
    FakeWebSocket.latest.receive(readyFrame(10));
    const before = api.calls.filter((call) => call.path.endsWith('/messages')).length;
    FakeWebSocket.latest.receive(eventFrame(11, 'another-chat', true));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(api.calls.filter((call) => call.path.endsWith('/messages')).length).toBe(before);
  });

  it('announces reconnecting and reloads the messages once live again', async () => {
    const { api } = openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await waitFor(() => {
      expect(FakeWebSocket.instances.length).toBeGreaterThan(0);
    });
    const socket = FakeWebSocket.latest;
    socket.receive(readyFrame(10));
    await waitFor(() => {
      expect(screen.getByTestId('chat-connection')).toHaveTextContent('Live');
    });
    const status = screen.getByTestId('chat-connection');
    expect(status).toHaveAttribute('role', 'status');
    expect(status).toHaveAttribute('aria-live', 'polite');
    socket.serverClose();
    await waitFor(() => {
      expect(screen.getByTestId('chat-connection')).toHaveTextContent('Reconnecting…');
    });
    const loads = api.calls.filter((call) => call.path === '/api/chat/sessions/s1/messages').length;
    await waitFor(
      () => {
        expect(FakeWebSocket.instances.length).toBeGreaterThan(1);
      },
      { timeout: 3000 },
    );
    expect(FakeWebSocket.latest.url).toMatch(/[?]after=[0-9]+$/);
    FakeWebSocket.latest.receive(readyFrame(10));
    await waitFor(() => {
      expect(
        api.calls.filter((call) => call.path === '/api/chat/sessions/s1/messages').length,
      ).toBeGreaterThan(loads);
    });
    expect(screen.getByTestId('chat-connection')).toHaveTextContent('Live');
  });

  it('reloads on a resync', async () => {
    const { api } = openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await waitFor(() => {
      expect(FakeWebSocket.instances.length).toBeGreaterThan(0);
    });
    FakeWebSocket.latest.receive(readyFrame(10));
    const loads = api.calls.filter((call) => call.path.endsWith('/messages')).length;
    FakeWebSocket.latest.receive({ type: 'resync', latest_event_id: 99 });
    await waitFor(() => {
      expect(api.calls.filter((call) => call.path.endsWith('/messages')).length).toBeGreaterThan(
        loads,
      );
    });
  });

  it('closes the connection when the page is left', async () => {
    const user = userEvent.setup();
    openSession();
    renderApp('/chats/s1');
    await screen.findByTestId('chat-log');
    await waitFor(() => {
      expect(FakeWebSocket.instances.length).toBeGreaterThan(0);
    });
    const socket = FakeWebSocket.latest;
    await user.click(screen.getByRole('link', { name: 'All chats' }));
    await waitFor(() => {
      expect(socket.closedByClient).toBe(1000);
    });
  });
});
