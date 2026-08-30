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

    // WEBGL_debug_renderer_info exposes the *real* GPU string
    // (e.g. "ANGLE (Qualcomm, Adreno (TM) 730, OpenGL ES 3.2)"). The plain
    // RENDERER/VENDOR parameters are masked to "WebKit WebGL" / "WebKit" on
    // many browsers, which defeats every renderer-based check below. Privacy
    // builds (Brave, Firefox with resistFingerprinting, Tor, Safari 17+)
    // withhold this extension or return a generic value — fall back to the
    // masked parameters, then 'unknown', and never treat a withheld value as
    // suspicious (see the software-renderer check, which only fires on an
    // explicit software/VM string).
    const dbg = glContext.getExtension('WEBGL_debug_renderer_info');
    const renderer =
      (dbg && glContext.getParameter(dbg.UNMASKED_RENDERER_WEBGL)) ||
      glContext.getParameter(glContext.RENDERER) ||
      'unknown';
    const vendor =
      (dbg && glContext.getParameter(dbg.UNMASKED_VENDOR_WEBGL)) ||
      glContext.getParameter(glContext.VENDOR) ||
      'unknown';
    return { renderer: String(renderer), vendor: String(vendor) };
  } catch {
    return { renderer: 'unknown', vendor: 'unknown' };
  }
}

export function detectEmulation(): { isEmulation: boolean; inconsistencies: string[]; metrics: DeviceMetrics } {
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

  // Software / VM renderers. SwiftShader has never backed WebGL on a real
  // phone (Chromium docs: "SwiftShader is not used on mobile") and Chrome is
  // removing the desktop fallback too (deprecated M130, removed ~M137 — the
  // context now just fails to create), so on this mobile-only flow an
  // explicit software string means an emulator, a headless browser, or a VM
  // — not a legitimate device.
  //
  // Deliberately NOT matched (these were false positives for real users):
  //  - bare "Mesa": every real Linux/ChromeOS Chrome with Intel/AMD hardware
  //    reports "ANGLE (Intel, Mesa Intel(R) UHD Graphics ...)" — only Mesa's
  //    headless "Mesa OffScreen" surface is a software tell.
  //  - bare "Gallium": appears as "Gallium 0.4 on llvmpipe" (caught by
  //    llvmpipe) but also "Gallium 0.4 on AMD ..." on real hardware.
  //  - bare "Software": too broad.
  const softwareRendererPatterns =
    /SwiftShader|Software Rasterizer|Microsoft Basic Render|Mesa OffScreen|\bllvmpipe\b|SwiftShader Device|Subzero|VirtualBox|VMware|VirGL|Android Emulator/i;
  const isSoftwareRenderer = softwareRendererPatterns.test(webglInfo.renderer);

  // "Google Inc. (Google)" as the unmasked vendor — i.e. no hardware vendor
  // named — is the documented headless-Chrome signature. Real GPUs report
  // "Google Inc. (Qualcomm)", "Google Inc. (ARM)", "Google Inc. (Apple)",
  // "Apple GPU" (Safari), "Qualcomm", etc. Matched exactly, not as a
  // substring: older Chrome returned a bare "Google Inc." even with real
  // hardware, so that form must not count.
  const vendorNamesNoHardware = webglInfo.vendor.trim() === 'Google Inc. (Google)';

  if (isSoftwareRenderer) {
    inconsistencies.push(`Software/VM renderer detected: ${webglInfo.renderer}`);
  } else if (vendorNamesNoHardware && webglInfo.renderer !== 'unknown') {
    inconsistencies.push(`WebGL reports no hardware GPU (vendor: ${webglInfo.vendor})`);
  }
  const isEmulatorGPU = isSoftwareRenderer || vendorNamesNoHardware;
  
  if (claimsMobileUA && isDesktopGPU && !isMobileGPU) {
    inconsistencies.push('Desktop GPU detected with mobile User-Agent');
  }
  
  if (claimsMobileUA && hasFinePointer && !hasCoarsePointer && maxTouchPoints === 0) {
    inconsistencies.push('Mobile UA but fine pointer with no touch (desktop mouse/keyboard)');
  }
  
  if (/iPhone|iPad/.test(ua) && !/Safari/.test(ua)) {
    inconsistencies.push('Claims iOS but missing Safari signature (spoofed UA)');
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

  if (deviceMemory !== null) {
    const roundedPatterns = [2, 4, 8, 16, 32];
    if (roundedPatterns.includes(deviceMemory) && isEmulatorGPU) {
      inconsistencies.push(`Device memory exactly ${deviceMemory}GB with emulator GPU`);
    }
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
