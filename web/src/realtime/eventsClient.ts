// One WebSocket per tab for the event feed. It remembers the last event ID, reconnects with
// exponential backoff and jitter asking the server to replay from that ID, and tells subscribers
// to reload their views after a resync or a reconnect.

export type ConnectionState = 'connecting' | 'live' | 'reconnecting' | 'stopped';

export interface FeedEvent {
  id: number;
  kind: string;
  chatSessionId: string;
  messageId: string;
  authorIsAgent: boolean;
}

export type ClientEvent =
  | { type: 'event'; event: FeedEvent }
  | { type: 'resync' }
  | { type: 'reconnected' }
  | { type: 'state'; state: ConnectionState };

export interface SocketLike {
  onopen: ((event: unknown) => void) | null;
  onmessage: ((event: { data: unknown }) => void) | null;
  onerror: ((event: unknown) => void) | null;
  onclose: ((event: { code: number }) => void) | null;
  close: (code?: number) => void;
}

export interface EventsClientDeps {
  createSocket: (url: string) => SocketLike;
  setTimer: (callback: () => void, milliseconds: number) => unknown;
  clearTimer: (handle: unknown) => void;
  random: () => number;
  /** Where to connect: derived from the page address by default. */
  baseUrl: () => string;
  /** Asked when a connection closes before it was ever ready: is the sign-in still valid? */
  sessionStillValid: () => Promise<boolean>;
}

export const INITIAL_DELAY_MS = 1000;
export const MAX_DELAY_MS = 30_000;

/** Backoff for the Nth consecutive failure: 1 s doubling to 30 s, with 50–100% jitter. */
export function backoffDelay(attempt: number, random: number): number {
  const base = Math.min(MAX_DELAY_MS, INITIAL_DELAY_MS * 2 ** attempt);
  return Math.round(base * (0.5 + random * 0.5));
}

export function defaultBaseUrl(): string {
  const { protocol, host } = window.location;
  return `${protocol === 'https:' ? 'wss:' : 'ws:'}//${host}/api/events`;
}

function parseFrame(data: unknown): ClientEvent | { type: 'ready'; latest: number } | null {
  if (typeof data !== 'string') {
    return null;
  }
  let frame: unknown;
  try {
    frame = JSON.parse(data);
  } catch {
    return null;
  }
  if (typeof frame !== 'object' || frame === null || !('type' in frame)) {
    return null;
  }
  const value = frame as Record<string, unknown>;
  if (value['type'] === 'ready' && typeof value['latest_event_id'] === 'number') {
    return { type: 'ready', latest: value['latest_event_id'] };
  }
  if (value['type'] === 'resync') {
    return { type: 'resync' };
  }
  if (
    value['type'] === 'event' &&
    typeof value['id'] === 'number' &&
    typeof value['kind'] === 'string' &&
    typeof value['chat_session_id'] === 'string' &&
    typeof value['message_id'] === 'string'
  ) {
    return {
      type: 'event',
      event: {
        id: value['id'],
        kind: value['kind'],
        chatSessionId: value['chat_session_id'],
        messageId: value['message_id'],
        authorIsAgent: value['author_is_agent'] === true,
      },
    };
  }
  return null;
}

export class EventsClient {
  private readonly deps: EventsClientDeps;
  private readonly listeners = new Set<(event: ClientEvent) => void>();
  private socket: SocketLike | null = null;
  private timer: unknown = null;
  private lastId: number | null = null;
  private attempt = 0;
  private everLive = false;
  private running = false;
  private current: ConnectionState = 'stopped';

  constructor(deps: EventsClientDeps) {
    this.deps = deps;
  }

  get state(): ConnectionState {
    return this.current;
  }

  get lastEventId(): number | null {
    return this.lastId;
  }

  subscribe(listener: (event: ClientEvent) => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  start(): void {
    if (this.running) {
      return;
    }
    this.running = true;
    this.everLive = false;
    this.attempt = 0;
    this.setState('connecting');
    this.open();
  }

  stop(): void {
    this.running = false;
    if (this.timer !== null) {
      this.deps.clearTimer(this.timer);
      this.timer = null;
    }
    const socket = this.socket;
    this.socket = null;
    if (socket !== null) {
      socket.onopen = null;
      socket.onmessage = null;
      socket.onerror = null;
      socket.onclose = null;
      socket.close(1000);
    }
    this.setState('stopped');
  }

  private emit(event: ClientEvent): void {
    for (const listener of [...this.listeners]) {
      listener(event);
    }
  }

  private setState(state: ConnectionState): void {
    if (this.current !== state) {
      this.current = state;
      this.emit({ type: 'state', state });
    }
  }

  private open(): void {
    const base = this.deps.baseUrl();
    const url = this.lastId === null ? base : `${base}?after=${String(this.lastId)}`;
    const socket = this.deps.createSocket(url);
    this.socket = socket;
    let ready = false;
    socket.onmessage = (message) => {
      const frame = parseFrame(message.data);
      if (frame === null) {
        return;
      }
      if (frame.type === 'ready') {
        ready = true;
        this.lastId ??= frame.latest;
        const wasReconnect = this.everLive;
        this.everLive = true;
        this.attempt = 0;
        this.setState('live');
        if (wasReconnect) {
          this.emit({ type: 'reconnected' });
        }
        return;
      }
      if (frame.type === 'resync') {
        this.emit(frame);
        return;
      }
      if (frame.type === 'event') {
        this.lastId = Math.max(this.lastId ?? 0, frame.event.id);
        this.emit(frame);
      }
    };
    socket.onclose = () => {
      if (this.socket !== socket) {
        return;
      }
      this.socket = null;
      if (!this.running) {
        return;
      }
      if (ready) {
        this.scheduleReconnect();
        return;
      }
      void this.deps.sessionStillValid().then(
        (valid) => {
          if (!this.running) {
            return;
          }
          if (valid) {
            this.scheduleReconnect();
          } else {
            this.stop();
          }
        },
        () => {
          if (this.running) {
            this.scheduleReconnect();
          }
        },
      );
    };
  }

  private scheduleReconnect(): void {
    this.setState('reconnecting');
    const delay = backoffDelay(this.attempt, this.deps.random());
    this.attempt += 1;
    this.timer = this.deps.setTimer(() => {
      this.timer = null;
      if (this.running) {
        this.open();
      }
    }, delay);
  }
}
