import '@testing-library/jest-dom';
import { vi } from 'vitest';

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
    if (typeof msg === 'string' && msg.includes('React Router Future Flag')) {
      return;
    }
    originalWarn.call(console, ...args);
  }),
};
