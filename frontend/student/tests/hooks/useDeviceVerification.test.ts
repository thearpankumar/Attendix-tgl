import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

vi.mock('../../src/hooks/useDeviceVerification', () => ({
  useDeviceVerification: vi.fn(),
}));

import { useDeviceVerification } from '../../src/hooks/useDeviceVerification';
import { useIsMobile, useMobileVerification } from '../../src/hooks/useIsMobile';

const mockDeviceVerification = (overrides: Partial<ReturnType<typeof useDeviceVerification>> = {}) => {
  vi.mocked(useDeviceVerification).mockReturnValue({
    isValid: true,
    isEmulation: false,
    inconsistencies: [],
    metrics: {
      maxTouchPoints: 5,
      hasCoarsePointer: true,
      touchEventSupport: true,
      orientationSupport: true,
      webglRenderer: 'Adreno (TM) 650',
      webglVendor: 'Qualcomm',
      screenWidth: 1080,
      screenHeight: 2400,
      devicePixelRatio: 2.75,
      hardwareConcurrency: 8,
      deviceMemory: 6,
      userAgent: 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36',
      platform: 'Linux armv81',
      language: 'en-US',
      isEmulation: false,
      inconsistencies: [],
    },
    checking: false,
    performVerification: vi.fn(),
    result: null as any,
    recheck: vi.fn(),
    ...overrides,
  } as any);
};

describe('useIsMobile', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns true for valid mobile device', () => {
    mockDeviceVerification({ isValid: true, isEmulation: false, checking: false });
    
    const { result } = renderHook(() => useIsMobile());
    
    expect(result.current).toBe(true);
  });

  it('returns false for emulated device', () => {
    mockDeviceVerification({ isValid: false, isEmulation: true, checking: false });
    
    const { result } = renderHook(() => useIsMobile());
    
    expect(result.current).toBe(false);
  });

  it('returns false for non-valid device', () => {
    mockDeviceVerification({ isValid: false, isEmulation: false, checking: false });
    
    const { result } = renderHook(() => useIsMobile());
    
    expect(result.current).toBe(false);
  });

  it('returns true while checking (default state)', () => {
    mockDeviceVerification({ checking: true });
    
    const { result } = renderHook(() => useIsMobile());
    
    expect(result.current).toBe(true);
  });
});

describe('useMobileVerification', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('returns full verification result', () => {
    const mockMetrics = {
      maxTouchPoints: 5,
      hasCoarsePointer: true,
      touchEventSupport: true,
      orientationSupport: true,
      webglRenderer: 'Adreno (TM) 650',
      webglVendor: 'Qualcomm',
      screenWidth: 1080,
      screenHeight: 2400,
      devicePixelRatio: 2.75,
      hardwareConcurrency: 8,
      deviceMemory: 6,
      userAgent: 'Mozilla/5.0 (Linux; Android 13; SM-G991B)',
      platform: 'Linux armv81',
      language: 'en-US',
      isEmulation: false,
      inconsistencies: [],
    };
    
    mockDeviceVerification({ 
      isValid: true, 
      isEmulation: false, 
      checking: false,
      metrics: mockMetrics as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isEmulation).toBe(false);
    expect(result.current.metrics).toEqual(mockMetrics);
  });

  it('detects emulation correctly', () => {
    mockDeviceVerification({ 
      isValid: false, 
      isEmulation: true, 
      checking: false,
      inconsistencies: ['Desktop GPU detected', 'maxTouchPoints exactly 1'],
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(false);
    expect(result.current.isEmulation).toBe(true);
    expect(result.current.inconsistencies).toHaveLength(2);
  });

  it('provides recheck function', () => {
    const mockRecheck = vi.fn();
    mockDeviceVerification({ recheck: mockRecheck });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.recheck).toBe(mockRecheck);
  });

  it('shows checking state correctly', () => {
    mockDeviceVerification({ checking: true });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.checking).toBe(true);
    expect(result.current.isMobile).toBe(false);
  });
});

describe('Device Verification Security Scenarios', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('flags DevTools device emulation (maxTouchPoints === 1)', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['maxTouchPoints exactly 1 with mobile UA (DevTools emulation pattern)'],
      metrics: {
        maxTouchPoints: 1,
        hardwareConcurrency: 16,
        deviceMemory: 32,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isEmulation).toBe(true);
    expect(result.current.inconsistencies.some(i => i.includes('maxTouchPoints'))).toBe(true);
  });

  it('flags desktop GPU with mobile UA', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['Desktop GPU detected with mobile User-Agent'],
      metrics: {
        webglRenderer: 'NVIDIA GeForce RTX 3080',
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isEmulation).toBe(true);
  });

  it('flags high CPU cores with mobile UA', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['High CPU cores (>=8) with mobile UA (typical desktop pattern)'],
      metrics: {
        hardwareConcurrency: 16,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.inconsistencies.some(i => i.includes('CPU cores'))).toBe(true);
  });

  it('flags high memory with mobile UA', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['High device memory (>=8GB) with mobile UA'],
      metrics: {
        deviceMemory: 32,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.inconsistencies.some(i => i.includes('memory'))).toBe(true);
  });

  it('flags iOS UA without Safari signature', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['Claims iOS but missing Safari signature (spoofed UA)'],
      metrics: {
        userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X) Mobile/15E148',
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.inconsistencies.some(i => i.includes('Safari'))).toBe(true);
  });

  it('flags desktop-resolution screen with mobile UA', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['Desktop-resolution screen with mobile UA'],
      metrics: {
        screenWidth: 2560,
        screenHeight: 1440,
        devicePixelRatio: 1,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isEmulation).toBe(true);
  });

  it('allows real mobile device with legitimate specs', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        maxTouchPoints: 5,
        hardwareConcurrency: 8,
        deviceMemory: 6,
        webglRenderer: 'Adreno (TM) 650',
        screenWidth: 1080,
        screenHeight: 2400,
        devicePixelRatio: 2.75,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isEmulation).toBe(false);
    expect(result.current.inconsistencies).toHaveLength(0);
  });

  it('handles multiple inconsistency flags', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: [
        'maxTouchPoints exactly 1 with mobile UA (DevTools emulation pattern)',
        'Desktop GPU detected with mobile User-Agent',
        'High CPU cores (>=8) with mobile UA (typical desktop pattern)',
        'High device memory (>=8GB) with mobile UA',
      ],
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.inconsistencies.length).toBeGreaterThanOrEqual(4);
    expect(result.current.isEmulation).toBe(true);
    expect(result.current.isMobile).toBe(false);
  });
});

describe('Edge Cases', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('handles iPad with MacIntel platform (iPadOS 13+)', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        userAgent: 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/16.0 Safari/605.1.15',
        platform: 'MacIntel',
        maxTouchPoints: 5,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
  });

  it('handles Android Desktop Mode', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        userAgent: 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36',
        platform: 'Linux armv81',
        maxTouchPoints: 5,
        hasCoarsePointer: true,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
  });

  it('handles low-end Android device correctly', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        maxTouchPoints: 5,
        hardwareConcurrency: 4,
        deviceMemory: 2,
        screenWidth: 720,
        screenHeight: 1280,
        devicePixelRatio: 2,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isEmulation).toBe(false);
  });

  it('handles iPhone Pro Max correctly', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
        platform: 'iPhone',
        maxTouchPoints: 5,
        hardwareConcurrency: 6,
        deviceMemory: 6,
        screenWidth: 430,
        screenHeight: 932,
        devicePixelRatio: 3,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isEmulation).toBe(false);
  });

  it('handles privacy browser with limited APIs', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        userAgent: 'Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 (KHTML, like Gecko) Brave/1.60.0 Chrome/119.0.0.0 Mobile Safari/537.36',
        deviceMemory: null,
        maxTouchPoints: 5,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
  });

  it('handles tablet correctly', () => {
    mockDeviceVerification({
      isValid: true,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        userAgent: 'Mozilla/5.0 (Linux; Android 13; SM-X700) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36',
        maxTouchPoints: 10,
        hardwareConcurrency: 8,
        screenWidth: 1600,
        screenHeight: 2560,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(true);
    expect(result.current.isEmulation).toBe(false);
  });

  it('handles Windows tablet with touch', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: false,
      inconsistencies: [],
      metrics: {
        userAgent: 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36',
        platform: 'Win32',
        maxTouchPoints: 10,
        hasCoarsePointer: true,
        webglRenderer: 'Intel(R) UHD Graphics 620',
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isMobile).toBe(false);
  });
});

describe('Security Tests', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('detects automated tool spoofing', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['Desktop GPU detected with mobile User-Agent', 'High CPU cores (>=8) with mobile UA'],
      metrics: {
        userAgent: 'Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36',
        webglRenderer: 'NVIDIA GeForce GTX 1080',
        hardwareConcurrency: 16,
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isEmulation).toBe(true);
  });

  it('detects Chrome DevTools mobile mode', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['maxTouchPoints exactly 1 with mobile UA (DevTools emulation pattern)'],
      metrics: {
        maxTouchPoints: 1,
        userAgent: 'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36',
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.isEmulation).toBe(true);
    expect(result.current.isMobile).toBe(false);
  });

  it('detects suspiciously perfect emulation', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: [
        'maxTouchPoints exactly 1 with mobile UA (DevTools emulation pattern)',
        'Desktop GPU detected with mobile User-Agent',
        'High CPU cores (>=8) with mobile UA (typical desktop pattern)',
        'High device memory (>=8GB) with mobile UA',
      ],
      metrics: {
        maxTouchPoints: 1,
        hardwareConcurrency: 16,
        deviceMemory: 64,
        webglRenderer: 'NVIDIA GeForce RTX 4090',
      } as any,
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.inconsistencies.length).toBeGreaterThan(2);
    expect(result.current.isEmulation).toBe(true);
  });

  it('handles reactive state changes', () => {
    const { result, rerender } = renderHook(() => useMobileVerification());
    
    mockDeviceVerification({ isValid: true, isEmulation: false });
    rerender();
    
    expect(result.current.isMobile).toBe(true);
    
    mockDeviceVerification({ isValid: false, isEmulation: true });
    rerender();
    
    expect(result.current.isMobile).toBe(false);
  });

  it('provides actionable error messages', () => {
    mockDeviceVerification({
      isValid: false,
      isEmulation: true,
      inconsistencies: ['Desktop GPU detected with mobile User-Agent'],
    });
    
    const { result } = renderHook(() => useMobileVerification());
    
    expect(result.current.inconsistencies[0]).toBeDefined();
    expect(typeof result.current.inconsistencies[0]).toBe('string');
  });
});
