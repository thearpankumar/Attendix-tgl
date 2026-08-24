// Collected once, in the popup (a real document — service workers have no
// `window`/`navigator.userAgentData`/`screen`/WebRTC), at the moment
// pairing completes. This is deliberately NOT the full
// backend-rust/src/middleware/emulator_detection.rs logic ported to the
// client — that's server-side, defense-in-depth signal correlation across
// many submissions; this is a lightweight, best-effort client-side heuristic
// only, explicitly scoped down for Phase 3 (see the plan's Part A note).

export interface DeviceFingerprint {
  platform: string;
  screenWidth: number;
  screenHeight: number;
  colorDepth: number;
  hardwareConcurrency: number;
  deviceMemory: number | null;
  timezone: string;
  languages: readonly string[];
  webglRenderer: string | null;
}

export function collectDeviceFingerprint(): DeviceFingerprint {
  return {
    platform: navigator.platform,
    screenWidth: screen.width,
    screenHeight: screen.height,
    colorDepth: screen.colorDepth,
    hardwareConcurrency: navigator.hardwareConcurrency,
    deviceMemory: (navigator as Navigator & { deviceMemory?: number }).deviceMemory ?? null,
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    languages: navigator.languages,
    webglRenderer: readWebglRenderer(),
  };
}

function readWebglRenderer(): string | null {
  try {
    const canvas = document.createElement('canvas');
    const gl = (canvas.getContext('webgl') ?? canvas.getContext('experimental-webgl')) as
      | WebGLRenderingContext
      | null;
    if (!gl) return null;
    const ext = gl.getExtension('WEBGL_debug_renderer_info');
    if (!ext) return null;
    return gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) as string;
  } catch {
    return null;
  }
}

/** SHA-256 of a stable JSON serialization of the fingerprint — a compact
 * value to send as `deviceFingerprintHash`, not the raw fingerprint itself. */
export async function hashFingerprint(fp: DeviceFingerprint): Promise<string> {
  const json = JSON.stringify(fp, Object.keys(fp).sort());
  const bytes = new TextEncoder().encode(json);
  const digest = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

export interface HighEntropyUaData {
  platform?: string;
  platformVersion?: string;
  architecture?: string;
  bitness?: string;
  model?: string;
  uaFullVersion?: string;
}

/** `navigator.userAgentData` is Chromium-only and the high-entropy call is
 * async + permissioned — best-effort, returns null on unsupported browsers. */
export async function collectHighEntropyUaData(): Promise<HighEntropyUaData | null> {
  const uaData = (navigator as Navigator & {
    userAgentData?: {
      getHighEntropyValues: (hints: string[]) => Promise<Record<string, unknown>>;
    };
  }).userAgentData;
  if (!uaData) return null;
  try {
    const values = await uaData.getHighEntropyValues([
      'platform',
      'platformVersion',
      'architecture',
      'bitness',
      'model',
      'uaFullVersion',
    ]);
    return {
      platform: values.platform as string | undefined,
      platformVersion: values.platformVersion as string | undefined,
      architecture: values.architecture as string | undefined,
      bitness: values.bitness as string | undefined,
      model: values.model as string | undefined,
      uaFullVersion: values.uaFullVersion as string | undefined,
    };
  } catch {
    return null;
  }
}

/** Detects multiple monitors via the (Chromium-only, permissioned)
 * Window Management API where available, falling back to a `screen.isExtended`
 * check, and finally to "unknown" (not "false") rather than guessing wrong. */
export async function detectMultiMonitor(): Promise<boolean | null> {
  const withWm = window as Window & {
    getScreenDetails?: () => Promise<{ screens: unknown[] }>;
  };
  if (typeof withWm.getScreenDetails === 'function') {
    try {
      const details = await withWm.getScreenDetails();
      return details.screens.length > 1;
    } catch {
      // Permission not granted — fall through to the weaker signal below
      // rather than failing the whole pairing flow over an optional check.
    }
  }
  const isExtended = (screen as Screen & { isExtended?: boolean }).isExtended;
  return typeof isExtended === 'boolean' ? isExtended : null;
}

/** WebRTC local/public IP leak check: gathers ICE candidates from a
 * throwaway RTCPeerConnection against no real remote peer — this is purely
 * for reading the candidate addresses the browser exposes, not for
 * establishing a connection. A STUN server is required to get `srflx`
 * (server-reflexive/public) candidates — without one, ICE gathering only
 * ever produces `host` (local-network) candidates and this never actually
 * surfaces the public IP the anti-VPN signal needs. Resolves once ICE
 * gathering completes or after a short timeout, whichever comes first. */
export function collectWebrtcIceCandidates(timeoutMs = 1500): Promise<string[]> {
  return new Promise((resolve) => {
    const candidates = new Set<string>();
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      pc.close();
      resolve(Array.from(candidates));
    };

    const pc = new RTCPeerConnection({ iceServers: [{ urls: 'stun:stun.l.google.com:19302' }] });
    pc.onicecandidate = (event) => {
      if (!event.candidate) {
        finish();
        return;
      }
      const match = /candidate:\S+ \d+ \S+ \d+ (\S+) /.exec(event.candidate.candidate);
      const address = match?.[1];
      if (address) candidates.add(address);
    };
    pc.createDataChannel('ice-probe');
    pc
      .createOffer()
      .then((offer) => pc.setLocalDescription(offer))
      .catch(finish);

    setTimeout(finish, timeoutMs);
  });
}

/** Lightweight, best-effort automation/emulator heuristic — a small set of
 * signals that are individually weak but collectively suggestive. Returns
 * the specific flags that fired, not just a boolean, so the admin can see
 * *why* a device was flagged. Not a replacement for the backend's
 * `emulator_detection.rs` (which correlates across many submissions
 * server-side); this is client-side and scoped down deliberately. */
export function detectAutomationFlags(fp: DeviceFingerprint): string[] {
  const flags: string[] = [];
  if ((navigator as Navigator & { webdriver?: boolean }).webdriver) {
    flags.push('navigator.webdriver');
  }
  if (fp.hardwareConcurrency === 0) {
    flags.push('zero_hardware_concurrency');
  }
  if (fp.webglRenderer && /swiftshader|llvmpipe|software/i.test(fp.webglRenderer)) {
    flags.push('software_renderer');
  }
  if (fp.screenWidth === 0 || fp.screenHeight === 0) {
    flags.push('zero_screen_dimensions');
  }
  return flags;
}
