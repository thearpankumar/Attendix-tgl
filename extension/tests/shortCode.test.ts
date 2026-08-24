import { describe, it, expect } from 'vitest';
import { detectShortCodeFromUrl } from '../lib/shortCode';

describe('detectShortCodeFromUrl', () => {
  it('detects a code from /s/{code} (the link an admin actually shares)', () => {
    expect(detectShortCodeFromUrl('https://attendix.example.com/s/abc123')?.shortCode).toBe('abc123');
  });

  it('detects a code from /attend/{code} — the URL a real browser settles on after Caddy/backend redirect /s/ there', () => {
    expect(detectShortCodeFromUrl('https://attendix.example.com/attend/abc123')?.shortCode).toBe('abc123');
  });

  it('does NOT treat /attend/pair/{code}/{pairingCode} as a session to pair — that is the phone-side pairing ceremony page', () => {
    expect(detectShortCodeFromUrl('https://attendix.example.com/attend/pair/abc123/xyz789')).toBeNull();
  });

  it('does NOT treat /attend/legacy/{code} as a session to pair — that is the legacy attendance flow', () => {
    expect(detectShortCodeFromUrl('https://attendix.example.com/attend/legacy/abc123')).toBeNull();
  });

  it('returns null for an unrelated URL', () => {
    expect(detectShortCodeFromUrl('https://attendix.example.com/admin/sessions')).toBeNull();
  });

  it('returns null for an undefined URL', () => {
    expect(detectShortCodeFromUrl(undefined)).toBeNull();
  });

  it('apiBase always comes from the configured API_BASE_URL, never the tab origin', () => {
    const result = detectShortCodeFromUrl('https://some-other-origin.example.com/attend/abc123');
    expect(result?.apiBase).not.toContain('some-other-origin');
  });
});
