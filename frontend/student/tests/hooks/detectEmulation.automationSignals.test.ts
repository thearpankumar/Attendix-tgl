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

  it('flags a Chrome UA with zero plugins', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [],
      languages: ['en-US'],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('zero plugins'))).toBe(true);
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

  it('flags empty navigator.languages', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      maxTouchPoints: 5,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: [],
      platform: 'Linux armv81',
    });

    const { inconsistencies } = detectEmulation();
    expect(inconsistencies.some((i) => i.includes('Empty navigator.languages'))).toBe(true);
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
