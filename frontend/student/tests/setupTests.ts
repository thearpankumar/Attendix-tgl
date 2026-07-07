import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock WebAuthn API
if (!window.PublicKeyCredential) {
  // @ts-ignore
  window.PublicKeyCredential = {
    isUserVerifyingPlatformAuthenticatorAvailable: vi.fn().mockResolvedValue(true),
  };
}
if (!navigator.credentials) {
  // @ts-ignore
  navigator.credentials = {
    create: vi.fn(),
    get: vi.fn(),
  };
}

// Mock Geolocation API
if (!navigator.geolocation) {
  // @ts-ignore
  navigator.geolocation = {
    getCurrentPosition: vi.fn(),
  };
}

// Mock MediaDevices API (Camera)
if (!navigator.mediaDevices) {
  // @ts-ignore
  navigator.mediaDevices = {
    getUserMedia: vi.fn().mockResolvedValue({
      getTracks: () => [{ stop: vi.fn() }],
    }),
  };
}

// Mock global fetch
globalThis.fetch = vi.fn();

// Mock DOMParser for SVG parsing
class MockDOMParser {
  parseFromString() {
    const div = document.createElement('div');
    return {
      documentElement: div
    };
  }
}
(globalThis as any).DOMParser = MockDOMParser as any;

// Mock HTMLMediaElement.play
window.HTMLMediaElement.prototype.play = vi.fn().mockResolvedValue(undefined);
