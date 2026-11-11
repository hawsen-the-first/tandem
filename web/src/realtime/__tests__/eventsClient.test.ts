import { beforeEach, describe, expect, it, vi } from 'vitest';
import { EventsClient, MAX_DELAY_MS, backoffDelay, defaultBaseUrl } from '../eventsClient';
import type { ClientEvent, EventsClientDeps } from '../eventsClient';
import { FakeWebSocket, eventFrame, readyFrame } from './fakeWebSocket';

interface Timer {
  id: number;
  at: number;
  run: () => void;
}

function harness(overrides: Partial<EventsClientDeps> = {}) {
  let now = 0;
  let nextId = 1;
  let timers: Timer[] = [];
  const deps: EventsClientDeps = {
    createSocket: (url) => new FakeWebSocket(url),
    setTimer: (run, milliseconds) => {
      const timer = { id: nextId++, at: now + milliseconds, run };
      timers.push(timer);
      return timer.id;
    },
    clearTimer: (handle) => {
      timers = timers.filter((timer) => timer.id !== handle);
    },
    random: () => 1,
    baseUrl: () => 'ws://example.test/api/events',
    sessionStillValid: () => Promise.resolve(true),
    ...overrides,
  };
  const client = new EventsClient(deps);
  const seen: ClientEvent[] = [];
  client.subscribe((event) => seen.push(event));
  return {
    client,
    seen,
    pending: () => timers.map((timer) => timer.at - now),
    advance: (milliseconds: number) => {
      now += milliseconds;
      const due = timers.filter((timer) => timer.at <= now);
      timers = timers.filter((timer) => timer.at > now);
      due.forEach((timer) => {
        timer.run();
      });
    },
    states: () => seen.flatMap((event) => (event.type === 'state' ? [event.state] : [])),
  };
}

beforeEach(() => {
  FakeWebSocket.reset();
});

describe('backoff', () => {
  it('doubles from one second up to thirty seconds', () => {
    const delays = [0, 1, 2, 3, 4, 5, 6, 10].map((attempt) => backoffDelay(attempt, 1));
    expect(delays).toEqual([1000, 2000, 4000, 8000, 16_000, 30_000, 30_000, 30_000]);
    expect(MAX_DELAY_MS).toBe(30_000);
  });

  it('applies jitter between half and all of the delay', () => {
    expect(backoffDelay(2, 0)).toBe(2000);
    expect(backoffDelay(2, 0.5)).toBe(3000);
    expect(backoffDelay(2, 1)).toBe(4000);
  });
});

describe('connection URL', () => {
  it('uses ws for http pages and wss for https pages', () => {
    expect(defaultBaseUrl()).toMatch(/^ws:\/\/[^/]+\/api\/events$/);
  });
});

describe('EventsClient', () => {
  it('connects, becomes live on ready and remembers the last event ID', () => {
    const { client, states } = harness();
    client.start();
    expect(states()).toEqual(['connecting']);
    expect(FakeWebSocket.latest.url).toBe('ws://example.test/api/events');
    FakeWebSocket.latest.open();
    FakeWebSocket.latest.receive(readyFrame(41));
    expect(client.state).toBe('live');
    expect(client.lastEventId).toBe(41);
    FakeWebSocket.latest.receive(eventFrame(42, 's1'));
    expect(client.lastEventId).toBe(42);
  });

  it('delivers events with their fields mapped', () => {
    const { client, seen } = harness();
    client.start();
    FakeWebSocket.latest.receive(readyFrame(0));
    FakeWebSocket.latest.receive(eventFrame(7, 's1', true));
    expect(seen).toContainEqual({
      type: 'event',
      event: {
        id: 7,
        kind: 'message.posted',
        chatSessionId: 's1',
        messageId: 'm-7',
        authorIsAgent: true,
      },
    });
  });

  it('ignores frames it does not understand', () => {
    const { client, seen } = harness();
    client.start();
    FakeWebSocket.latest.receive(readyFrame(0));
    const before = seen.length;
    FakeWebSocket.latest.receiveRaw('not json');
    FakeWebSocket.latest.receiveRaw(new ArrayBuffer(2));
    FakeWebSocket.latest.receive({ type: 'mystery' });
    FakeWebSocket.latest.receive({ type: 'event', id: 'x' });
    expect(seen).toHaveLength(before);
  });

  it('reconnects with backoff, asking to replay from the last event, and reports it', () => {
    const { client, seen, advance, pending, states } = harness();
    client.start();
    FakeWebSocket.latest.receive(readyFrame(5));
    FakeWebSocket.latest.receive(eventFrame(6, 's1'));
    FakeWebSocket.latest.serverClose();
    expect(client.state).toBe('reconnecting');
    expect(pending()).toEqual([1000]);
    advance(999);
    expect(FakeWebSocket.instances).toHaveLength(1);
    advance(1);
    expect(FakeWebSocket.instances).toHaveLength(2);
    expect(FakeWebSocket.latest.url).toBe('ws://example.test/api/events?after=6');
    FakeWebSocket.latest.receive(readyFrame(6));
    expect(client.state).toBe('live');
    expect(seen).toContainEqual({ type: 'reconnected' });
    expect(states()).toEqual(['connecting', 'live', 'reconnecting', 'live']);
  });

  it('grows the delay while connections keep failing and resets it once live', async () => {
    const { client, advance, pending } = harness();
    client.start();
    FakeWebSocket.latest.receive(readyFrame(1));
    FakeWebSocket.latest.serverClose();
    advance(1000);
    FakeWebSocket.latest.serverClose();
    await Promise.resolve();
    await Promise.resolve();
    expect(pending()).toEqual([2000]);
    advance(2000);
    FakeWebSocket.latest.receive(readyFrame(1));
    FakeWebSocket.latest.serverClose();
    expect(pending()).toEqual([1000]);
  });

  it('passes a resync on so views reload, without reconnecting', () => {
    const { client, seen } = harness();
    client.start();
    FakeWebSocket.latest.receive(readyFrame(1));
    FakeWebSocket.latest.receive({ type: 'resync', latest_event_id: 90 });
    expect(seen).toContainEqual({ type: 'resync' });
    expect(FakeWebSocket.instances).toHaveLength(1);
  });

  it('stops when a connection that never became ready belongs to an ended sign-in', async () => {
    const check = vi.fn().mockResolvedValue(false);
    const { client, pending } = harness({ sessionStillValid: check });
    client.start();
    FakeWebSocket.latest.serverClose();
    await vi.waitFor(() => {
      expect(client.state).toBe('stopped');
    });
    expect(check).toHaveBeenCalledOnce();
    expect(pending()).toEqual([]);
  });

  it('retries a connection that failed before ready when the sign-in is still valid', async () => {
    const { client, advance } = harness();
    client.start();
    FakeWebSocket.latest.serverClose();
    await vi.waitFor(() => {
      expect(client.state).toBe('reconnecting');
    });
    advance(1000);
    expect(FakeWebSocket.instances).toHaveLength(2);
  });

  it('keeps retrying when the sign-in check itself fails', async () => {
    const { client } = harness({ sessionStillValid: () => Promise.reject(new Error('offline')) });
    client.start();
    FakeWebSocket.latest.serverClose();
    await vi.waitFor(() => {
      expect(client.state).toBe('reconnecting');
    });
  });

  it('closes the socket and cancels any reconnect when stopped', () => {
    const { client, pending } = harness();
    client.start();
    const first = FakeWebSocket.latest;
    first.receive(readyFrame(1));
    client.stop();
    expect(first.closedByClient).toBe(1000);
    expect(client.state).toBe('stopped');

    client.start();
    FakeWebSocket.latest.receive(readyFrame(1));
    FakeWebSocket.latest.serverClose();
    expect(pending()).toHaveLength(1);
    client.stop();
    expect(pending()).toEqual([]);
  });

  it('ignores a second start and stops notifying unsubscribed listeners', () => {
    const { client, seen } = harness();
    const extra = vi.fn();
    const unsubscribe = client.subscribe(extra);
    client.start();
    client.start();
    expect(FakeWebSocket.instances).toHaveLength(1);
    unsubscribe();
    FakeWebSocket.latest.receive(readyFrame(1));
    expect(extra).toHaveBeenCalledTimes(1);
    expect(seen.length).toBeGreaterThan(1);
  });

  it('does not reconnect after stop even if the old socket reports a close', () => {
    const { client } = harness();
    client.start();
    const socket = FakeWebSocket.latest;
    client.stop();
    socket.serverClose();
    expect(FakeWebSocket.instances).toHaveLength(1);
  });
});
