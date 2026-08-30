import { describe, it, expect, vi, afterEach } from 'vitest';
import { postEvents, refreshTelemetryToken, finishPairing } from '../lib/api';
import type { TelemetryEvent } from '../lib/types';

function mockFetchOnce(status: number, body: unknown = {}) {
  const fetchMock = vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    json: () => Promise.resolve(body),
  });
  vi.stubGlobal('fetch', fetchMock);
  return fetchMock;
}

const SAMPLE_EVENTS: TelemetryEvent[] = [
  { eventType: 'heartbeat', eventData: {}, recordedAt: new Date().toISOString() },
];

describe('postEvents', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('sends the telemetry token as an Authorization: Bearer header', async () => {
    const fetchMock = mockFetchOnce(202);

    const result = await postEvents(
      'https://api.example.com',
      'ABC123',
      'ext-1',
      'ROLL001',
      SAMPLE_EVENTS,
      'signed-token-value',
    );

    expect(result).toBe(true);
    const [, options] = fetchMock.mock.calls[0]!;
    expect(options.headers.authorization).toBe('Bearer signed-token-value');
  });

  it('omits the Authorization header when no token is supplied', async () => {
    const fetchMock = mockFetchOnce(202);

    await postEvents('https://api.example.com', 'ABC123', 'ext-1', 'ROLL001', SAMPLE_EVENTS, '');

    const [, options] = fetchMock.mock.calls[0]!;
    expect(options.headers.authorization).toBeUndefined();
  });

  it('returns false on a 403 (device no longer locked / stale token)', async () => {
    mockFetchOnce(403);

    const result = await postEvents(
      'https://api.example.com',
      'ABC123',
      'ext-1',
      'ROLL001',
      SAMPLE_EVENTS,
      'a-token',
    );

    expect(result).toBe(false);
  });

  it('throws on other non-ok statuses', async () => {
    mockFetchOnce(500);

    await expect(
      postEvents('https://api.example.com', 'ABC123', 'ext-1', 'ROLL001', SAMPLE_EVENTS, 'a-token'),
    ).rejects.toThrow('Failed to post telemetry batch (500)');
  });
});

describe('refreshTelemetryToken', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('returns the reissued token on success', async () => {
    mockFetchOnce(200, {
      telemetryToken: 'new-token',
      telemetryTokenExpiresAt: '2026-01-01T00:00:00Z',
    });

    const result = await refreshTelemetryToken('https://api.example.com', 'ABC123', 'old-token');

    expect(result).toEqual({
      telemetryToken: 'new-token',
      telemetryTokenExpiresAt: '2026-01-01T00:00:00Z',
    });
  });

  it('sends the current token as the Authorization header', async () => {
    const fetchMock = mockFetchOnce(200, {
      telemetryToken: 'new-token',
      telemetryTokenExpiresAt: '2026-01-01T00:00:00Z',
    });

    await refreshTelemetryToken('https://api.example.com', 'ABC123', 'old-token');

    const [, options] = fetchMock.mock.calls[0]!;
    expect(options.headers.authorization).toBe('Bearer old-token');
  });

  it('returns null when the lock is no longer active (403)', async () => {
    mockFetchOnce(403);
    const result = await refreshTelemetryToken('https://api.example.com', 'ABC123', 'old-token');
    expect(result).toBeNull();
  });

  it('returns null when the token has fully expired (401)', async () => {
    mockFetchOnce(401);
    const result = await refreshTelemetryToken('https://api.example.com', 'ABC123', 'old-token');
    expect(result).toBeNull();
  });

  it('throws on other non-ok statuses', async () => {
    mockFetchOnce(500);
    await expect(
      refreshTelemetryToken('https://api.example.com', 'ABC123', 'old-token'),
    ).rejects.toThrow('Failed to refresh telemetry token (500)');
  });
});

describe('finishPairing', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('returns the telemetry token fields from the response', async () => {
    mockFetchOnce(200, {
      locked: true,
      rollNumber: 'ROLL001',
      telemetryToken: 'issued-token',
      telemetryTokenExpiresAt: '2026-01-01T00:30:00Z',
    });

    const result = await finishPairing(
      'https://api.example.com',
      'ABC123',
      'PAIRCODE1',
      'fingerprint-hash',
      'Chrome',
      'Windows',
    );

    expect(result.telemetryToken).toBe('issued-token');
    expect(result.telemetryTokenExpiresAt).toBe('2026-01-01T00:30:00Z');
  });
});
