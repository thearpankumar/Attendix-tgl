import { describe, it, expect, afterEach } from 'vitest';

window.matchMedia = window.matchMedia || function () {
  return {
    matches: false,
    addListener: function () {},
    removeListener: function () {},
  } as unknown as MediaQueryList;
};

import { detectEmulation } from '../../src/hooks/useDeviceVerification';

// A realistic, non-emulated Android/Chrome mobile profile — the baseline
// every test below starts from, flipping exactly one signal at a time so a
// failure points at the right check.
const ANDROID_CHROME_UA =
  'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36';

function setNavigatorProps(props: Record<string, unknown>) {
  for (const [key, value] of Object.entries(props)) {
    Object.defineProperty(navigator, key, { value, writable: true, configurable: true });
  }
}

describe('detectEmulation — automation-signal defense-in-depth', () => {
  afterEach(() => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US', 'en'],
      language: 'en-US',
      platform: 'Linux armv81',
    });
  });

  // BUG FIX (2026-08-30): Chrome for Android intentionally returns
  // navigator.plugins.length === 0 as a privacy measure (Chrome 57+, MDN-
  // documented). Flagging it as emulation blocked every real Android Chrome
  // user. The check has been removed from detectEmulation() entirely.
  it('does NOT flag Chrome UA with zero plugins (real Android Chrome behaviour)', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [],
      languages: ['en-US'],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('zero plugins'))).toBe(false);
  });

  it('does not flag zero plugins on a non-Chrome UA (e.g. Firefox)', () => {
    setNavigatorProps({
      userAgent: 'Mozilla/5.0 (Android 13; Mobile; rv:120.0) Gecko/120.0 Firefox/120.0',
      maxTouchPoints: 5,
      plugins: [],
      languages: ['en-US'],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('zero plugins'))).toBe(false);
  });

  it('does not flag a real Chrome mobile browser with plugins present', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US'],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('zero plugins'))).toBe(false);
  });

  // BUG FIX (2026-08-30): navigator.languages.length === 0 was removed from
  // the blocking inconsistencies[] because privacy-focused browsers legitimately
  // suppress the languages list. The backend already excluded this from its
  // hard-reject gate for the same reason.
  it('does NOT flag empty navigator.languages (privacy browser behaviour)', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: [],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('Empty navigator.languages'))).toBe(false);
  });

  it('does not flag a real device with populated languages', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US', 'en'],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('Empty navigator.languages'))).toBe(false);
  });
});
