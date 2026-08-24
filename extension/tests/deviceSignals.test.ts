import { describe, it, expect } from 'vitest';
import { detectAutomationFlags, hashFingerprint, type DeviceFingerprint } from '../lib/deviceSignals';

const BASE_FINGERPRINT: DeviceFingerprint = {
  platform: 'Win32',
  screenWidth: 1920,
  screenHeight: 1080,
  colorDepth: 24,
  hardwareConcurrency: 8,
  deviceMemory: 8,
  timezone: 'Asia/Kolkata',
  languages: ['en-US'],
  webglRenderer: 'ANGLE (NVIDIA GeForce GTX 1080)',
};

describe('detectAutomationFlags', () => {
  it('flags nothing for an ordinary fingerprint', () => {
    expect(detectAutomationFlags(BASE_FINGERPRINT)).toEqual([]);
  });

  it('flags navigator.webdriver', () => {
    Object.defineProperty(navigator, 'webdriver', { value: true, configurable: true });
    expect(detectAutomationFlags(BASE_FINGERPRINT)).toContain('navigator.webdriver');
    Object.defineProperty(navigator, 'webdriver', { value: false, configurable: true });
  });

  it('flags zero hardware concurrency', () => {
    const fp = { ...BASE_FINGERPRINT, hardwareConcurrency: 0 };
    expect(detectAutomationFlags(fp)).toContain('zero_hardware_concurrency');
  });

  it('flags a software WebGL renderer (common in headless/emulated environments)', () => {
    const fp = { ...BASE_FINGERPRINT, webglRenderer: 'Google SwiftShader' };
    expect(detectAutomationFlags(fp)).toContain('software_renderer');
  });

  it('flags zero screen dimensions', () => {
    const fp = { ...BASE_FINGERPRINT, screenWidth: 0, screenHeight: 0 };
    expect(detectAutomationFlags(fp)).toContain('zero_screen_dimensions');
  });

  it('can report multiple flags at once', () => {
    const fp = { ...BASE_FINGERPRINT, hardwareConcurrency: 0, screenWidth: 0, screenHeight: 0 };
    expect(detectAutomationFlags(fp).length).toBe(2);
  });
});

describe('hashFingerprint', () => {
  it('is deterministic for the same fingerprint', async () => {
    const a = await hashFingerprint(BASE_FINGERPRINT);
    const b = await hashFingerprint(BASE_FINGERPRINT);
    expect(a).toBe(b);
    expect(a).toMatch(/^[0-9a-f]{64}$/);
  });

  it('produces a different hash when a field changes', async () => {
    const a = await hashFingerprint(BASE_FINGERPRINT);
    const b = await hashFingerprint({ ...BASE_FINGERPRINT, hardwareConcurrency: 4 });
    expect(a).not.toBe(b);
  });

  it('is insensitive to object key order (stable serialization)', async () => {
    const reordered = {
      webglRenderer: BASE_FINGERPRINT.webglRenderer,
      languages: BASE_FINGERPRINT.languages,
      timezone: BASE_FINGERPRINT.timezone,
      deviceMemory: BASE_FINGERPRINT.deviceMemory,
      hardwareConcurrency: BASE_FINGERPRINT.hardwareConcurrency,
      colorDepth: BASE_FINGERPRINT.colorDepth,
      screenHeight: BASE_FINGERPRINT.screenHeight,
      screenWidth: BASE_FINGERPRINT.screenWidth,
      platform: BASE_FINGERPRINT.platform,
    } as DeviceFingerprint;
    const a = await hashFingerprint(BASE_FINGERPRINT);
    const b = await hashFingerprint(reordered);
    expect(a).toBe(b);
  });
});
