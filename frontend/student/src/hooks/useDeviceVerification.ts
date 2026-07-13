import { useState, useEffect, useCallback } from 'react';

export interface DeviceMetrics {
  maxTouchPoints: number;
  hasCoarsePointer: boolean;
  touchEventSupport: boolean;
  orientationSupport: boolean;
  webglRenderer: string;
  webglVendor: string;
  screenWidth: number;
  screenHeight: number;
  devicePixelRatio: number;
  hardwareConcurrency: number;
  deviceMemory: number | null;
  userAgent: string;
  platform: string;
  language: string;
  isEmulation: boolean;
  inconsistencies: string[];
}

export interface DeviceCheckResult {
  isValid: boolean;
  isEmulation: boolean;
  inconsistencies: string[];
  metrics: DeviceMetrics;
  reason?: string;
}

function getWebGLInfo(): { renderer: string; vendor: string } {
  try {
    const canvas = document.createElement('canvas');
    const gl = canvas.getContext('webgl') || canvas.getContext('experimental-webgl');
    if (!gl) return { renderer: 'unknown', vendor: 'unknown' };
    
    const glContext = gl as WebGLRenderingContext;
    const renderer = glContext.getParameter(glContext.RENDERER) || 'unknown';
    const vendor = glContext.getParameter(glContext.VENDOR) || 'unknown';
    return { renderer, vendor };
  } catch {
    return { renderer: 'unknown', vendor: 'unknown' };
  }
}

function detectEmulation(): { isEmulation: boolean; inconsistencies: string[]; metrics: DeviceMetrics } {
  const inconsistencies: string[] = [];
  
  const webglInfo = getWebGLInfo();
  const ua = navigator.userAgent;
  const claimsMobileUA = /iPhone|iPad|iPod|Android|Mobile/i.test(ua);
  const claimsDesktopUA = /Windows|Macintosh|Linux|X11/i.test(ua) && !claimsMobileUA;
  
  const maxTouchPoints = navigator.maxTouchPoints || 0;
  const hasCoarsePointer = window.matchMedia('(pointer: coarse)').matches;
  const hasFinePointer = window.matchMedia('(pointer: fine)').matches;
  const touchEventSupport = 'ontouchstart' in window;
  const orientationSupport = 'orientation' in screen;
  const screenWidth = window.screen.width;
  const screenHeight = window.screen.height;
  const devicePixelRatio = window.devicePixelRatio || 1;
  const hardwareConcurrency = navigator.hardwareConcurrency || 1;
  const deviceMemory = (navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? null;
  const platform = navigator.platform || '';
  const language = navigator.language || '';
  
  const desktopGPUPatterns = /NVIDIA|AMD|Radeon|GeForce|Intel.*Graphics|RTX|GTX|Arc/i;
  const isDesktopGPU = desktopGPUPatterns.test(webglInfo.renderer);
  const isMobileGPU = /Adreno|Mali|PowerVR|Apple GPU|Intel.* HD Graphics/i.test(webglInfo.renderer);
  
  if (maxTouchPoints === 1 && claimsMobileUA) {
    inconsistencies.push('maxTouchPoints exactly 1 with mobile UA (DevTools emulation pattern)');
  }
  
  if (claimsMobileUA && isDesktopGPU && !isMobileGPU) {
    inconsistencies.push('Desktop GPU detected with mobile User-Agent');
  }
  
  if (claimsMobileUA && hardwareConcurrency >= 8) {
    inconsistencies.push('High CPU cores (>=8) with mobile UA (typical desktop pattern)');
  }
  
  if (claimsMobileUA && deviceMemory && deviceMemory >= 8) {
    inconsistencies.push('High device memory (>=8GB) with mobile UA');
  }
  
  if (claimsMobileUA && hasFinePointer && !hasCoarsePointer && maxTouchPoints === 0) {
    inconsistencies.push('Mobile UA but fine pointer with no touch (desktop mouse/keyboard)');
  }
  
  if (/iPhone|iPad/.test(ua) && !/Safari/.test(ua)) {
    inconsistencies.push('Claims iOS but missing Safari signature (spoofed UA)');
  }
  
  if (/iPad/.test(ua) && platform === 'MacIntel' && maxTouchPoints === 0) {
    inconsistencies.push('Claims iPad but MacIntel platform with no touch');
  }
  
  if (/Android/.test(ua) && !/Linux/.test(ua) && !/Mobile/.test(ua)) {
    inconsistencies.push('Claims Android but missing Linux/Mobile patterns');
  }
  
  if (screenWidth > 1920 && claimsMobileUA && !/iPad/.test(ua)) {
    inconsistencies.push('Desktop-resolution screen with mobile UA');
  }
  
  if (claimsDesktopUA && maxTouchPoints > 0 && hasCoarsePointer) {
    inconsistencies.push('Desktop UA but touch device detected (possible emulation having UA issues)');
  }
  
  if (devicePixelRatio === 1 && claimsMobileUA && !/iPad/.test(ua)) {
    inconsistencies.push('DPI ratio of 1 with mobile UA (unusual for phones)');
  }

  const metrics: DeviceMetrics = {
    maxTouchPoints,
    hasCoarsePointer,
    touchEventSupport,
    orientationSupport,
    webglRenderer: webglInfo.renderer,
    webglVendor: webglInfo.vendor,
    screenWidth,
    screenHeight,
    devicePixelRatio,
    hardwareConcurrency,
    deviceMemory,
    userAgent: ua,
    platform,
    language,
    isEmulation: inconsistencies.length > 0,
    inconsistencies,
  };

  return { isEmulation: inconsistencies.length > 0, inconsistencies, metrics };
}

export function useDeviceVerification() {
  const [result, setResult] = useState<DeviceCheckResult>({
    isValid: false,
    isEmulation: false,
    inconsistencies: [],
    metrics: null as unknown as DeviceMetrics,
  });
  const [checking, setChecking] = useState(true);

  const performVerification = useCallback(async (): Promise<DeviceCheckResult> => {
    const { isEmulation, inconsistencies, metrics } = detectEmulation();
    
    setChecking(false);
    
    return {
      isValid: !isEmulation && (metrics.maxTouchPoints > 0 || metrics.hasCoarsePointer),
      isEmulation,
      inconsistencies,
      metrics,
      reason: isEmulation ? 'Device emulation detected. Please use a real mobile device.' : undefined,
    };
  }, []);

  useEffect(() => {
    performVerification().then(setResult);
  }, [performVerification]);

  return { ...result, checking, recheck: () => performVerification().then(setResult) };
}

export function useIsRealMobile() {
  const { isValid, isEmulation, inconsistencies, checking, metrics } = useDeviceVerification();
  
  const isMobile = !checking && isValid && !isEmulation;
  
  return { 
    isMobile, 
    isEmulation, 
    inconsistencies,
    checking,
    metrics,
  };
}
