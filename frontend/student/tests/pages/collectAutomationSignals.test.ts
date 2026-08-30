import { describe, it, expect, afterEach } from 'vitest';

window.matchMedia = window.matchMedia || function () {
  return {
    matches: false,
    addListener: function () {},
    removeListener: function () {},
  } as unknown as MediaQueryList;
};

import { collectAutomationSignals } from '../../src/pages/StudentScan';

const ANDROID_CHROME_UA =
  'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36';

function setNavigatorProps(props: Record<string, unknown>) {
  for (const [key, value] of Object.entries(props)) {
    Object.defineProperty(navigator, key, { value, writable: true, configurable: true });
  }
}

function resetDocumentCdcMarkers() {
  for (const key of Object.keys(document)) {
    if (key.startsWith('$cdc_') || key.startsWith('$wdc_')) {
      delete (document as unknown as Record<string, unknown>)[key];
    }
  }
}

describe('collectAutomationSignals', () => {
  afterEach(() => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US', 'en'],
      webdriver: false,
    });
    resetDocumentCdcMarkers();
  });

  it('returns an empty list for a real, un-automated mobile browser', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US', 'en'],
      webdriver: false,
    });

    expect(collectAutomationSignals()).toEqual([]);
  });

  it('flags navigator.webdriver === true', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US'],
      webdriver: true,
    });

    expect(collectAutomationSignals()).toContain('webdriver');
  });

  it('flags a Selenium-injected $cdc_ marker on document', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: ['en-US'],
      webdriver: false,
    });
    (document as unknown as Record<string, unknown>).$cdc_asdjflasutopfhvcZLmcfl_ = true;

    expect(collectAutomationSignals()).toContain('cdc-markers');
  });

  it('flags a Chrome UA with zero plugins as informational-only', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [],
      languages: ['en-US'],
      webdriver: false,
    });

    expect(collectAutomationSignals()).toContain('no-plugins-chrome');
  });

  it('flags empty navigator.languages as informational-only', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [{ name: 'Chrome PDF Viewer' }],
      languages: [],
      webdriver: false,
    });

    expect(collectAutomationSignals()).toContain('empty-languages');
  });

  it('can report multiple signals at once', () => {
    setNavigatorProps({
      userAgent: ANDROID_CHROME_UA,
      plugins: [],
      languages: [],
      webdriver: true,
    });

    const signals = collectAutomationSignals();
    expect(signals).toContain('webdriver');
    expect(signals).toContain('no-plugins-chrome');
    expect(signals).toContain('empty-languages');
  });
});
