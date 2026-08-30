import { describe, it, expect, afterEach, vi } from 'vitest';

window.matchMedia = window.matchMedia || function () {
  return {
    matches: false,
    addListener: function () {},
    removeListener: function () {},
  } as unknown as MediaQueryList;
};

import { detectEmulation } from '../../src/hooks/useDeviceVerification';

const ANDROID_CHROME_UA =
  'Mozilla/5.0 (Linux; Android 13; SM-G991B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Mobile Safari/537.36';

function setNavigatorProps(props: Record<string, unknown>) {
  for (const [key, value] of Object.entries(props)) {
    Object.defineProperty(navigator, key, { value, writable: true, configurable: true });
  }
}

const REAL_MOBILE_NAVIGATOR = {
  userAgent: ANDROID_CHROME_UA,
  maxTouchPoints: 5,
  plugins: [{ name: 'Chrome PDF Viewer' }],
  languages: ['en-US', 'en'],
  language: 'en-US',
  platform: 'Linux armv81',
};

const UNMASKED_RENDERER = 0x9246;
const UNMASKED_VENDOR = 0x9245;
const PLAIN_RENDERER = 0x1f01;
const PLAIN_VENDOR = 0x1f00;

/** Install a fake WebGL context so getWebGLInfo() sees a given renderer/vendor.
 *  `debugExt: false` simulates a privacy build that withholds
 *  WEBGL_debug_renderer_info — only the masked params are then readable. */
function mockWebGL(opts: {
  unmaskedRenderer?: string;
  unmaskedVendor?: string;
  maskedRenderer?: string;
  maskedVendor?: string;
  debugExt?: boolean;
  noContext?: boolean;
}) {
  const {
    unmaskedRenderer = '',
    unmaskedVendor = '',
    maskedRenderer = 'WebKit WebGL',
    maskedVendor = 'WebKit',
    debugExt = true,
    noContext = false,
  } = opts;

  const fakeGl = {
    RENDERER: PLAIN_RENDERER,
    VENDOR: PLAIN_VENDOR,
    getExtension: (name: string) =>
      name === 'WEBGL_debug_renderer_info' && debugExt
        ? { UNMASKED_RENDERER_WEBGL: UNMASKED_RENDERER, UNMASKED_VENDOR_WEBGL: UNMASKED_VENDOR }
        : null,
    getParameter: (p: number) => {
      if (p === UNMASKED_RENDERER) return unmaskedRenderer;
      if (p === UNMASKED_VENDOR) return unmaskedVendor;
      if (p === PLAIN_RENDERER) return maskedRenderer;
      if (p === PLAIN_VENDOR) return maskedVendor;
      return '';
    },
  };

  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
    ((ctx: string) =>
      !noContext && (ctx === 'webgl' || ctx === 'experimental-webgl')
        ? (fakeGl as unknown as RenderingContext)
        : null) as typeof HTMLCanvasElement.prototype.getContext,
  );
}

const softwareInc = (inc: string[]) =>
  inc.some((i) => i.includes('Software/VM renderer') || i.includes('no hardware GPU'));

describe('detectEmulation — WebGL renderer heuristic', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
  });

  it('does not flag a real Android device (Adreno via ANGLE)', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({
      unmaskedRenderer: 'ANGLE (Qualcomm, Adreno (TM) 730, OpenGL ES 3.2)',
      unmaskedVendor: 'Google Inc. (Qualcomm)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  it('does not flag a real Android device (bare Mali string, no ANGLE)', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({ unmaskedRenderer: 'Mali-G78', unmaskedVendor: 'ARM' });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  // The regression this change is about: real Linux / ChromeOS Chrome on
  // Intel or AMD hardware reports a "Mesa ..." renderer. That must not read
  // as a software renderer.
  it('does not flag real Linux desktop Chrome on Intel/Mesa', () => {
    setNavigatorProps({ ...REAL_MOBILE_NAVIGATOR, platform: 'Linux x86_64' });
    mockWebGL({
      unmaskedRenderer: 'ANGLE (Intel, Mesa Intel(R) UHD Graphics (TGL GT2), OpenGL 4.6)',
      unmaskedVendor: 'Google Inc. (Intel)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  it('does not flag "Gallium 0.4 on AMD" (real Mesa hardware path)', () => {
    setNavigatorProps({ ...REAL_MOBILE_NAVIGATOR, platform: 'Linux x86_64' });
    mockWebGL({
      unmaskedRenderer: 'AMD Radeon RX 6700 XT (Gallium 0.4, DRM 3.49, LLVM 15.0.7)',
      unmaskedVendor: 'Google Inc. (AMD)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  it('does not flag a privacy build that withholds WEBGL_debug_renderer_info', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({ debugExt: false, maskedRenderer: 'WebKit WebGL', maskedVendor: 'WebKit' });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  it('does not flag when WebGL is unavailable entirely', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({ noContext: true });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  it('does not flag older Chrome that reports a bare "Google Inc." vendor', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({
      unmaskedRenderer: 'ANGLE (NVIDIA GeForce GTX 1060 Direct3D11 vs_5_0 ps_5_0)',
      unmaskedVendor: 'Google Inc.',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(false);
  });

  it('flags classic "Google SwiftShader" software rendering', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({ unmaskedRenderer: 'Google SwiftShader', unmaskedVendor: 'Google Inc.' });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(true);
  });

  it('flags headless Chrome (SwiftShader via ANGLE/Vulkan)', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({
      unmaskedRenderer:
        'ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)',
      unmaskedVendor: 'Google Inc. (Google)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(true);
  });

  it('flags the "Google Inc. (Google)" vendor signature even without a SwiftShader string', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({
      unmaskedRenderer: 'ANGLE (Google, Vulkan 1.3.0, llvmpipe)',
      unmaskedVendor: 'Google Inc. (Google)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(true);
  });

  it('flags llvmpipe software rasterizer', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({
      unmaskedRenderer: 'ANGLE (Mesa, llvmpipe (LLVM 15.0.6, 256 bits), OpenGL 4.5)',
      unmaskedVendor: 'Google Inc. (Mesa)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(true);
  });

  it('flags the Android Studio emulator GLES translator', () => {
    setNavigatorProps(REAL_MOBILE_NAVIGATOR);
    mockWebGL({
      unmaskedRenderer: 'Android Emulator OpenGL ES Translator (Google SwiftShader)',
      unmaskedVendor: 'Google (Google)',
    });
    expect(softwareInc(detectEmulation().inconsistencies)).toBe(true);
  });
});
