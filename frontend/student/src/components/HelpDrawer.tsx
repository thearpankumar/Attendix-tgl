import { useState } from 'react';

type Step = 'loading' | 'permissions' | 'error' | 'rollInput' | 'webauthnAction' | 'form' | 'success';

interface Props {
  currentStep: Step;
}

type Tab = 'flow' | 'location' | 'camera' | 'browsers';

/* ── Shared SVG icon components ───────────────────────────── */
const sz = (n = 16) => ({ width: n, height: n });
const svg = (p: Record<string, unknown>) => ({ fill: 'none', strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const, strokeWidth: '2', ...p });

const IcoQr = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><path d="M14 14h2v2h-2zm4 0h3v3h-3zm0 4h3v3h-3zm-4 2h2v3h-2z"/></svg>;
const IcoHash = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><line x1="4" y1="9" x2="20" y2="9"/><line x1="4" y1="15" x2="20" y2="15"/><line x1="10" y1="3" x2="8" y2="21"/><line x1="16" y1="3" x2="14" y2="21"/></svg>;
const IcoFingerprint = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M12 10a2 2 0 0 0-2 2c0 1.02-.1 2.51-.26 4"/><path d="M14 13.12c0 2.38 0 6.38-1 8.88"/><path d="M17.29 21.02c.12-.6.43-2.3.5-3.02"/><path d="M2 12a10 10 0 0 1 18-6"/><path d="M2 16h.01"/><path d="M21.8 16c.2-2 .131-5.354 0-6"/><path d="M5 19.5C5.5 18 6 15 6 12a6 6 0 0 1 .34-2"/><path d="M8.65 22c.21-.66.45-1.32.57-2"/><path d="M9 6.8a6 6 0 0 1 9 5.2v2"/></svg>;
const IcoCamera = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3z"/><circle cx="12" cy="13" r="3"/></svg>;
const IcoCheckCircle = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>;
const IcoMapPin = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M21 10c0 7-9 13-9 13s-9-6-9-13a9 9 0 0 1 18 0z"/><circle cx="12" cy="10" r="3"/></svg>;
const IcoGlobe = () => <svg {...sz(16)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><circle cx="12" cy="12" r="10"/><line x1="2" y1="12" x2="22" y2="12"/><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/></svg>;
const IcoList = () => <svg {...sz(16)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>;
const IcoShield = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>;
const IcoSmartphone = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><rect x="5" y="2" width="14" height="20" rx="2" ry="2"/><line x1="12" y1="18" x2="12.01" y2="18"/></svg>;
const IcoLock = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg>;
const IcoBattery = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><rect x="1" y="6" width="18" height="12" rx="2" ry="2"/><line x1="23" y1="13" x2="23" y2="11"/></svg>;
const IcoAndroid = () => <svg {...sz(18)} viewBox="0 0 24 24" fill="currentColor"><path d="M17.523 15.341a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5Zm-11.046 0a.75.75 0 1 0 0-1.5.75.75 0 0 0 0 1.5ZM6.5 8h11a2 2 0 0 1 2 2v5.5a2 2 0 0 1-2 2h-11a2 2 0 0 1-2-2V10a2 2 0 0 1 2-2Zm-.354-1.207 1.06-1.768A.5.5 0 0 1 7.634 5h8.732a.5.5 0 0 1 .428.243l1.06 1.768"/></svg>;
const IcoApple = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M12 5C9.5 5 7 7 7 10c0 4 2.5 9 5 9s5-5 5-9c0-3-2.5-5-5-5z"/><path d="M12 5V2"/><path d="M9.5 3.5C9.5 3.5 10.5 5 12 5s2.5-1.5 2.5-1.5"/></svg>;
const IcoFirefox = () => <svg {...sz(18)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><circle cx="12" cy="12" r="10"/><path d="M12 8c-2.2 0-4 1.8-4 4s1.8 4 4 4 4-1.8 4-4"/><path d="M12 8V6M16 10l2-2M8 16l-1.5 1.5"/></svg>;
const IcoAlertTriangle = () => <svg {...sz(14)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>;
const IcoInfo = () => <svg {...sz(14)} viewBox="0 0 24 24" {...svg({ stroke: 'currentColor' })}><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>;

/* ── Data ─────────────────────────────────────────────────── */
const FLOW_STEPS = [
  { icon: <IcoQr />,          label: 'Scan QR or open link',      desc: 'Your instructor shares a session link' },
  { icon: <IcoHash />,        label: 'Enter your roll number',     desc: 'Type your roll number (e.g. 21CS042)' },
  { icon: <IcoFingerprint />, label: 'Verify identity',            desc: 'Use biometric / passkey — first time enroll your device' },
  { icon: <IcoCamera />,      label: 'Capture selfie + location',  desc: 'Take a photo and confirm your GPS location' },
  { icon: <IcoCheckCircle />, label: 'Attendance recorded!',       desc: "Done — you're marked present" },
];

interface PlatformHelp { platform: string; platformIcon: JSX.Element; steps: string[]; extra: string | null; }

const LOCATION_HELP: PlatformHelp[] = [
  {
    platform: 'Chrome on Android',
    platformIcon: <IcoAndroid />,
    steps: [
      'Tap the lock icon to the LEFT of the URL bar',
      'Tap "Permissions"',
      'Tap "Location" → select "Allow"',
      'Reload the page',
    ],
    extra: 'If not working: Settings → Apps → Chrome → Permissions → Location → Allow',
  },
  {
    platform: 'Safari on iPhone / iPad',
    platformIcon: <IcoApple />,
    steps: [
      'Open the Settings app',
      'Tap "Privacy & Security" → "Location Services"',
      'Ensure Location Services is ON',
      'Scroll down → tap "Safari Websites"',
      'Select "While Using the App"',
      'Enable "Precise Location" toggle',
    ],
    extra: 'If greyed out: Settings → Screen Time → Content & Privacy → Location Services → Allow Changes',
  },
  {
    platform: 'Chrome on iPhone / iPad',
    platformIcon: <IcoApple />,
    steps: [
      'Open the Settings app',
      'Scroll down and tap "Chrome"',
      'Tap "Location"',
      'Select "While Using the App"',
    ],
    extra: null,
  },
  {
    platform: 'Firefox on Android',
    platformIcon: <IcoFirefox />,
    steps: [
      'Tap the lock icon in the address bar',
      'Tap "Edit Site Permissions"',
      'Set Location to "Allow"',
    ],
    extra: null,
  },
];

interface SpecialBlock { icon: JSX.Element; title: string; color: string; bg: string; content: string; }

const LOCATION_SPECIAL: SpecialBlock[] = [
  {
    icon: <IcoShield />,
    title: 'GrapheneOS / LineageOS / DivestOS',
    color: '#b45309', bg: '#fffbeb',
    content: 'These privacy-focused Android OS versions use GPS-only location — no Wi-Fi or cell tower assistance. GPS can take 60–90 seconds and may fail indoors.\n\nFix: Step outside with a clear view of the sky. Enable "Network location" in Settings → Location → Location services. Install a UnifiedNLP location backend (CalyxOS: enable Mozilla Location Service in microG settings).',
  },
  {
    icon: <IcoSmartphone />,
    title: '/e/OS (Murena) — Fake Location Warning',
    color: '#b42318', bg: '#fef3f2',
    content: 'The built-in "Advanced Privacy" app can fake your GPS location. Attendance will fail with a wrong location.\n\nFix: Open the Advanced Privacy app → disable "Hide my location" / "Fake my location" before marking attendance.',
  },
  {
    icon: <IcoSmartphone />,
    title: 'Xiaomi MIUI / HyperOS',
    color: '#4f46e5', bg: '#eef2ff',
    content: 'MIUI has aggressive battery optimization that throttles GPS.\n\nFix: Settings → Battery → Battery Saver → Exceptions → Add Chrome. Also check Settings → Privacy → Permission Manager → Location → Chrome → Allow.',
  },
  {
    icon: <IcoSmartphone />,
    title: 'Samsung One UI',
    color: '#0d9488', bg: '#f0fdfa',
    content: 'Samsung allows only "Approximate Location" which may fail campus geofencing.\n\nFix: Settings → Privacy & security → Permission manager → Location → Chrome → Enable "Use precise location".',
  },
  {
    icon: <IcoLock />,
    title: 'VPN is Active',
    color: '#667085', bg: '#f0f2f7',
    content: 'A VPN can cause location detection to fail or report the wrong region.\n\nFix: Disable your VPN temporarily while marking attendance, then re-enable it after.',
  },
  {
    icon: <IcoBattery />,
    title: 'Battery Saver Mode',
    color: '#667085', bg: '#f0f2f7',
    content: 'Battery saver / power saving modes throttle GPS on most Android phones.\n\nFix: Disable Battery Saver (Settings → Battery → Battery Saver → OFF) before marking attendance.',
  },
  {
    icon: <IcoSmartphone />,
    title: 'Nothing Phone',
    color: '#334155', bg: '#f8fafc',
    content: 'Nothing OS sometimes hangs location requests if background apps are stuck.\n\nFix: Swipe down from the top notification panel, check for "Active apps" (or background apps), and close them. If location is still stuck, toggle Location off and on again in Quick Settings.',
  },
];

const CAMERA_HELP: PlatformHelp[] = [
  {
    platform: 'Chrome on Android',
    platformIcon: <IcoAndroid />,
    steps: [
      'Tap the lock icon to the LEFT of the URL bar',
      'Tap "Permissions"',
      'Tap "Camera" → select "Allow"',
      'Reload the page',
    ],
    extra: 'Also check: phone Quick Settings panel — swipe down and ensure "Camera access" toggle is ON.',
  },
  {
    platform: 'Safari on iPhone / iPad',
    platformIcon: <IcoApple />,
    steps: [
      'Open Safari and go to the attendance page',
      'Tap the "aA" icon in the address bar',
      'Tap "Website Settings"',
      'Set "Camera" to "Allow"',
    ],
    extra: 'iOS 17+: Camera in third-party browsers (Chrome, Firefox) is also supported.',
  },
  {
    platform: 'Chrome on iPhone / iPad',
    platformIcon: <IcoApple />,
    steps: [
      'Open the Settings app',
      'Scroll down and tap "Chrome"',
      'Tap "Camera"',
      'Enable the toggle',
    ],
    extra: 'Requires iOS 17 or later for camera access in Chrome.',
  },
  {
    platform: 'Xiaomi / Samsung — Global Camera Toggle',
    platformIcon: <IcoSmartphone />,
    steps: [
      'Swipe down from the top of the screen to open Quick Settings',
      'Look for a "Camera access" tile',
      'Tap it to toggle Camera access ON',
      'Then grant camera permission in the browser',
    ],
    extra: 'This global toggle overrides all per-app permissions. Must be ON first.',
  },
];

/* ── Chrome SVG logo ─────────────────────────────────────── */
const ChromeLogo = () => (
  <svg width="28" height="28" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
    <circle cx="50" cy="50" r="50" fill="#fff"/>
    <circle cx="50" cy="50" r="20" fill="#4285F4"/>
    <path d="M50 30h43.3A50 50 0 0 0 6.7 30z" fill="#EA4335"/>
    <path d="M6.7 30A50 50 0 0 0 28.3 93.3L50 50z" fill="#FBBC05"/>
    <path d="M28.3 93.3A50 50 0 0 0 93.3 70L50 50z" fill="#34A853"/>
    <circle cx="50" cy="50" r="16" fill="#fff"/>
    <circle cx="50" cy="50" r="12" fill="#4285F4"/>
  </svg>
);

/* ── Safari SVG logo ─────────────────────────────────────── */
const SafariLogo = () => (
  <svg width="28" height="28" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg">
    <defs>
      <linearGradient id="saf-grad" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop offset="0%" stopColor="#1e90ff"/>
        <stop offset="100%" stopColor="#007aff"/>
      </linearGradient>
    </defs>
    <circle cx="50" cy="50" r="50" fill="url(#saf-grad)"/>
    <circle cx="50" cy="50" r="38" fill="none" stroke="rgba(255,255,255,.3)" strokeWidth="2"/>
    <polygon points="50,20 58,50 50,44 42,50" fill="#FF3B30"/>
    <polygon points="50,80 42,50 50,56 58,50" fill="white"/>
  </svg>
);

/* ── Component ───────────────────────────────────────────── */
export default function HelpDrawer({ currentStep }: Props) {
  const [open, setOpen] = useState(false);
  const [tab, setTab] = useState<Tab>('flow');

  if (currentStep === 'loading' || currentStep === 'success') return null;

  const tabs: { id: Tab; label: string; icon: JSX.Element }[] = [
    { id: 'flow',     label: 'How It Works',  icon: <IcoList /> },
    { id: 'location', label: 'Location Help', icon: <IcoMapPin /> },
    { id: 'camera',   label: 'Camera Help',   icon: <IcoCamera /> },
    { id: 'browsers', label: 'Browser Tips',  icon: <IcoGlobe /> },
  ];

  return (
    <>
      {/* Floating help button */}
      <button
        className="help-fab"
        onClick={() => setOpen(true)}
        aria-label="Help and troubleshooting"
        type="button"
        title="Help & Troubleshoot"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="12" cy="12" r="10" />
          <path d="M12 16v-4" />
          <path d="M12 8h.01" />
        </svg>
      </button>

      {open && <div className="help-backdrop" onClick={() => setOpen(false)} />}

      <div className={`help-drawer${open ? ' open' : ''}`} role="dialog" aria-modal="true" aria-label="Help and troubleshooting">
        {/* Handle */}
        <div className="help-drawer-handle-wrap" onClick={() => setOpen(false)}>
          <div className="help-drawer-handle" />
        </div>

        {/* Header */}
        <div className="help-drawer-header">
          <div className="help-drawer-title">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#4f46e5" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10" /><path d="M12 16v-4M12 8h.01" />
            </svg>
            Help & Troubleshoot
          </div>
          <button className="help-drawer-close" onClick={() => setOpen(false)} aria-label="Close help" type="button">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
              <path d="M18 6 6 18M6 6l12 12" />
            </svg>
          </button>
        </div>

        {/* Tabs — SVG icons instead of emojis */}
        <div className="help-tabs">
          {tabs.map(t => (
            <button key={t.id} className={`help-tab${tab === t.id ? ' active' : ''}`} onClick={() => setTab(t.id)} type="button">
              <span className="help-tab-icon">{t.icon}</span>
              <span>{t.label}</span>
            </button>
          ))}
        </div>

        <div className="help-drawer-content">

          {/* ── Flow Tab ── */}
          {tab === 'flow' && (
            <div className="help-section">
              <p className="help-section-intro">Here's exactly how the attendance process works, step by step:</p>
              <div className="help-flow-list">
                {FLOW_STEPS.map((s, i) => (
                  <div className="help-flow-item" key={i}>
                    <div className="help-flow-num">{i + 1}</div>
                    <div className="help-flow-svg-icon">{s.icon}</div>
                    <div className="help-flow-text">
                      <div className="help-flow-label">{s.label}</div>
                      <div className="help-flow-desc">{s.desc}</div>
                    </div>
                  </div>
                ))}
              </div>
              <div className="help-note">
                <IcoInfo />
                You only need to enroll your device <strong>once</strong>. After that, just verify with your fingerprint or face.
              </div>
            </div>
          )}

          {/* ── Location Tab ── */}
          {tab === 'location' && (
            <div className="help-section">
              <p className="help-section-intro">Location confirms you're physically on campus. If it's blocked or failing, follow these steps:</p>

              {LOCATION_HELP.map((item, i) => (
                <div className="help-platform-block" key={i}>
                  <div className="help-platform-title">
                    <span className="help-platform-icon">{item.platformIcon}</span>
                    {item.platform}
                  </div>
                  <ol className="help-steps-list">
                    {item.steps.map((s, j) => <li key={j}>{s}</li>)}
                  </ol>
                  {item.extra && <div className="help-steps-extra">{item.extra}</div>}
                </div>
              ))}

              <div className="help-section-label" style={{ marginTop: 20 }}>Special Cases & Known Issues</div>
              {LOCATION_SPECIAL.map((item, i) => (
                <div className="help-special-block" key={i} style={{ '--special-color': item.color, '--special-bg': item.bg } as React.CSSProperties}>
                  <div className="help-special-title">
                    <span className="help-special-icon">{item.icon}</span>
                    {item.title}
                  </div>
                  <div className="help-special-content">{item.content}</div>
                </div>
              ))}
            </div>
          )}

          {/* ── Camera Tab ── */}
          {tab === 'camera' && (
            <div className="help-section">
              <p className="help-section-intro">Camera is needed for your selfie photo. If it's not working or access is blocked:</p>

              {CAMERA_HELP.map((item, i) => (
                <div className="help-platform-block" key={i}>
                  <div className="help-platform-title">
                    <span className="help-platform-icon">{item.platformIcon}</span>
                    {item.platform}
                  </div>
                  <ol className="help-steps-list">
                    {item.steps.map((s, j) => <li key={j}>{s}</li>)}
                  </ol>
                  {item.extra && <div className="help-steps-extra">{item.extra}</div>}
                </div>
              ))}

              <div className="help-note" style={{ marginTop: 16 }}>
                <IcoAlertTriangle />
                If another app (e.g. video call) is using the camera, close it first and refresh this page.
              </div>
            </div>
          )}

          {/* ── Browsers Tab ── */}
          {tab === 'browsers' && (
            <div className="help-section">
              <p className="help-section-intro">For the best experience, use one of these recommended browsers:</p>

              <div className="help-browser-card recommended">
                <div className="help-browser-icon"><ChromeLogo /></div>
                <div className="help-browser-info">
                  <div className="help-browser-name">Google Chrome <span className="help-browser-badge">Recommended</span></div>
                  <div className="help-browser-desc">Best compatibility with camera, location, and passkey features on Android and iOS 17+</div>
                </div>
              </div>

              <div className="help-browser-card recommended">
                <div className="help-browser-icon"><SafariLogo /></div>
                <div className="help-browser-info">
                  <div className="help-browser-name">Safari <span className="help-browser-badge">Recommended</span></div>
                  <div className="help-browser-desc">Best choice for iPhone and iPad users — full camera, location, and Face ID support</div>
                </div>
              </div>

              <div className="help-section-label" style={{ marginTop: 20 }}>Other Browsers — Known Issues</div>

              {[
                { name: 'Firefox (Android)',          desc: 'Location and camera work, but passkey/biometric support may be limited on older versions. Update to latest.' },
                { name: 'Samsung Internet',           desc: 'May have issues with passkey / WebAuthn. Recommend switching to Chrome on Samsung devices.' },
                { name: 'Mi Browser / MIUI Browser',  desc: 'Known issues with camera permissions. Switch to Chrome for a smooth experience.' },
                { name: 'Brave / Other Privacy Browsers', desc: 'Fingerprint protection features may interfere with biometric passkeys. Use Chrome or Safari.' },
              ].map(b => (
                <div className="help-browser-card" key={b.name}>
                  <div className="help-browser-icon-mono"><IcoGlobe /></div>
                  <div className="help-browser-info">
                    <div className="help-browser-name" style={{ color: '#667085' }}>{b.name}</div>
                    <div className="help-browser-desc">{b.desc}</div>
                  </div>
                </div>
              ))}

              <div className="help-note" style={{ marginTop: 16 }}>
                <IcoInfo />
                Private / Incognito mode does <strong>not</strong> block location or camera — it only prevents history from being saved.
              </div>
            </div>
          )}

        </div>
      </div>
    </>
  );
}
