import { format } from './messages';

// Text for the chat skeleton and home page.
const chatMessages = {
  homeTitle: 'Home',
  homeReminderTitle: 'Your recovery codes are running low',
  homeReminderBody: 'Generate a new set so you can still sign in if you lose your device.',
  homeReminderLink: 'Go to your account',
  homeRecentTitle: 'Recent chats',
  homeNewChat: 'New chat',
  homeNoChats: 'You have no chats yet.',
  homeViewAll: 'All chats',

  listTitle: 'Chats',
  listCreateTitle: 'Start a chat',
  listTitleLabel: 'Title',
  listTitleHint: 'Up to {max} characters',
  listCreate: 'Create chat',
  listCreating: 'Creating…',
  listEmpty: 'You have no chats yet. Start one above.',
  listCaption: 'Your chats',
  listColumnTitle: 'Title',
  listColumnCreated: 'Started',
  listOpen: 'Open',
  listLoadError: 'Your chats could not be loaded.',
  titleRequired: 'Enter a title',
  titleTooLong: 'Titles can be up to {max} characters',

  sessionLoadError: 'This chat could not be loaded.',
  sessionBack: 'All chats',
  sessionLog: 'Messages',
  sessionEmpty: 'No messages yet. Say something below.',
  composerLabel: 'Message',
  composerHint: 'Enter sends, Shift+Enter adds a new line.',
  composerSend: 'Send',
  composerSending: 'Sending…',
  composerRequired: 'Write a message first',
  composerTooLong: 'Messages can be up to {max} characters',
  composerRemaining: '{count} characters left',
  authorYou: 'You',
  connectionLive: 'Live',
  connectionConnecting: 'Connecting…',
  connectionReconnecting: 'Reconnecting…',
  connectionStopped: 'Not connected',
  agentWaiting: 'Waiting for a reply…',
} as const;

export type ChatMessageKey = keyof typeof chatMessages;

export function c(key: ChatMessageKey, values?: Record<string, string | number>): string {
  return format(chatMessages[key], values);
}

export const TITLE_MAX = 120;
export const BODY_MAX = 4000;
