import { useEffect, useRef, useSyncExternalStore } from 'react';
import { call, client } from '../api/client';
import { ApiError } from '../api/errors';
import { EventsClient, defaultBaseUrl } from './eventsClient';
import type { ClientEvent, ConnectionState } from './eventsClient';

let shared: EventsClient | null = null;
let users = 0;

async function sessionStillValid(): Promise<boolean> {
  try {
    await call(client.GET('/api/auth/me'));
    return true;
  } catch (error) {
    return !(error instanceof ApiError && error.status === 401);
  }
}

function sharedClient(): EventsClient {
  shared ??= new EventsClient({
    createSocket: (url) => new WebSocket(url) as never,
    setTimer: (callback, milliseconds) => window.setTimeout(callback, milliseconds),
    clearTimer: (handle) => {
      window.clearTimeout(handle as number);
    },
    random: () => Math.random(),
    baseUrl: defaultBaseUrl,
    sessionStillValid,
  });
  return shared;
}

function subscribeToState(notify: () => void): () => void {
  return sharedClient().subscribe((event) => {
    if (event.type === 'state') {
      notify();
    }
  });
}

function currentState(): ConnectionState {
  return shared?.state ?? 'stopped';
}

/**
 * Use the tab's single event connection while a component is mounted. The connection opens with
 * the first user and closes with the last. Returns its state for display ("stopped" before the
 * first connection attempt and after the sign-in ends).
 */
export function useEvents(onEvent: (event: ClientEvent) => void): ConnectionState {
  const handler = useRef(onEvent);
  useEffect(() => {
    handler.current = onEvent;
  });
  useEffect(() => {
    const events = sharedClient();
    users += 1;
    const unsubscribe = events.subscribe((event) => {
      if (event.type !== 'state') {
        handler.current(event);
      }
    });
    events.start();
    return () => {
      unsubscribe();
      users -= 1;
      if (users === 0) {
        events.stop();
      }
    };
  }, []);
  return useSyncExternalStore(subscribeToState, currentState);
}
