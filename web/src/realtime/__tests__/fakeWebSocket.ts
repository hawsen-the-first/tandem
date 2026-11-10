/** A WebSocket stand-in the test drives by hand. */
export class FakeWebSocket {
  static instances: FakeWebSocket[] = [];

  static reset(): void {
    FakeWebSocket.instances = [];
  }

  static get latest(): FakeWebSocket {
    const socket = FakeWebSocket.instances.at(-1);
    if (socket === undefined) {
      throw new Error('no socket has been created');
    }
    return socket;
  }

  readonly url: string;
  closedByClient: number | null = null;
  onopen: ((event: unknown) => void) | null = null;
  onmessage: ((event: { data: unknown }) => void) | null = null;
  onerror: ((event: unknown) => void) | null = null;
  onclose: ((event: { code: number }) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    FakeWebSocket.instances.push(this);
  }

  open(): void {
    this.onopen?.({});
  }

  receive(frame: unknown): void {
    this.onmessage?.({ data: JSON.stringify(frame) });
  }

  receiveRaw(data: unknown): void {
    this.onmessage?.({ data });
  }

  /** The server (or network) ends the connection. */
  serverClose(code = 1006): void {
    this.onclose?.({ code });
  }

  close(code = 1000): void {
    this.closedByClient = code;
  }
}

export const readyFrame = (latest: number) => ({ type: 'ready', latest_event_id: latest });

export const eventFrame = (id: number, sessionId: string, authorIsAgent = false) => ({
  type: 'event',
  id,
  kind: 'message.posted',
  chat_session_id: sessionId,
  message_id: `m-${String(id)}`,
  author_is_agent: authorIsAgent,
});
