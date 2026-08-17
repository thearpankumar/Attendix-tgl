import '@testing-library/jest-dom';
import { vi, beforeEach } from 'vitest';

// jsdom has no IntersectionObserver. This fake records every instance on
// globalThis so infinite-scroll tests can grab the latest one and fire its
// callback manually (`observer.callback([{ isIntersecting: true }])`) to
// simulate the sentinel scrolling into view.
globalThis.__intersectionObserverInstances = [];
class IntersectionObserverMock {
  constructor(callback, options) {
    this.callback = callback;
    this.options = options;
    globalThis.__intersectionObserverInstances.push(this);
  }
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.IntersectionObserver = IntersectionObserverMock;

beforeEach(() => {
  globalThis.__intersectionObserverInstances = [];
});

vi.mock('framer-motion', () => {
  const React = require('react');
  const makeTag = (tag) =>
    React.forwardRef(({ children, ...props }, ref) =>
      React.createElement(String(tag), { ...props, ref }, children)
    );
  return {
    motion: new Proxy({}, { get: (_t, tag) => makeTag(tag) }),
    AnimatePresence: ({ children }) => React.createElement(React.Fragment, null, children),
    useAnimation: () => ({ start: vi.fn(), stop: vi.fn() }),
    useMotionValue: (v) => ({ get: () => v, set: vi.fn() }),
    useTransform: (_v, _i, o) => ({ get: () => o[0] }),
  };
});

class LocalStorageMock {
  constructor() {
    this.store = {};
  }
  clear() {
    this.store = {};
  }
  getItem(key) {
    return this.store[key] || null;
  }
  setItem(key, value) {
    this.store[key] = String(value);
  }
  removeItem(key) {
    delete this.store[key];
  }
}

const mockLocalStorage = new LocalStorageMock();

try {
  delete globalThis.localStorage;
  delete global.localStorage;
} catch {
  // Ignored: delete might throw in some environments
}

Object.defineProperty(globalThis, 'localStorage', {
  value: mockLocalStorage,
  writable: true,
  configurable: true
});

Object.defineProperty(global, 'localStorage', {
  value: mockLocalStorage,
  writable: true,
  configurable: true
});

vi.mock('axios', () => {
  return {
    default: {
      get: vi.fn().mockResolvedValue({ data: {} }),
      post: vi.fn().mockResolvedValue({ data: {} }),
      put: vi.fn().mockResolvedValue({ data: {} }),
      delete: vi.fn().mockResolvedValue({ data: {} }),
      patch: vi.fn().mockResolvedValue({ data: {} }),
      defaults: {
        headers: {
          common: {}
        }
      }
    }
  };
});

vi.mock('react-toastify', () => {
  return {
    toast: {
      success: vi.fn(),
      error: vi.fn(),
      info: vi.fn(),
      warning: vi.fn(),
    },
    ToastContainer: () => null
  };
});

const originalError = console.error;
const originalWarn = console.warn;

global.console = {
  ...console,
  error: vi.fn((...args) => {
    const msg = args[0];
    if (
      typeof msg === 'string' &&
      (msg.includes('act(...)') ||
        msg.includes('Each child in a list should have a unique "key" prop') ||
        msg.includes('key" prop'))
    ) {
      return;
    }
    originalWarn.call(console, ...args);
  }),
  warn: vi.fn((...args) => {
    const msg = args[0];
    if (
      typeof msg === 'string' &&
      msg.includes('React Router Future Flag')
    ) {
      return;
    }
    originalWarn.call(console, ...args);
  }),
};
