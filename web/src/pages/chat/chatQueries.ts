import { call, client } from '../../api/client';
import type { components } from '../../api/schema';

export type ChatSession = components['schemas']['ChatSessionDto'];
export type ChatMessage = components['schemas']['ChatMessageDto'];

export const SESSIONS_KEY = ['chat', 'sessions'] as const;
export const messagesKey = (sessionId: string) => ['chat', 'messages', sessionId] as const;

export function fetchSessions(): Promise<ChatSession[]> {
  return call(client.GET('/api/chat/sessions'));
}

export function fetchMessages(sessionId: string): Promise<ChatMessage[]> {
  return call(
    client.GET('/api/chat/sessions/{id}/messages', { params: { path: { id: sessionId } } }),
  );
}

/** Add a message to a cached list unless it is already there (events and responses can race). */
export function appendUnique(messages: ChatMessage[] | undefined, message: ChatMessage) {
  const list = messages ?? [];
  return list.some((existing) => existing.id === message.id) ? list : [...list, message];
}
