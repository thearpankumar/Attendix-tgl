import { useEffect, useRef, useState } from 'react';
import QRCode from 'qrcode';
import { browser } from 'wxt/browser';
import {
  getOrCreateExtensionInstanceId,
  getConsentAcknowledged,
  setConsentAcknowledged,
  getPairingState,
  setPairingState,
  getPendingPairing,
  setPendingPairing,
} from '../../lib/storage';
import { startPairing, pollPairingStatus, finishPairing } from '../../lib/api';
import { detectShortCodeFromUrl } from '../../lib/shortCode';
import {
  collectDeviceFingerprint,
  hashFingerprint,
  collectHighEntropyUaData,
  detectMultiMonitor,
  detectAutomationFlags,
  collectWebrtcIceCandidates,
} from '../../lib/deviceSignals';
import type { PairingState } from '../../lib/types';
import './App.css';

type Screen = 'loading' | 'consent' | 'no-session' | 'pairing' | 'finishing' | 'paired' | 'error';

const POLL_INTERVAL_MS = 3000;

export default function App() {
  const [screen, setScreen] = useState<Screen>('loading');
  const [error, setError] = useState<string | null>(null);
  const [session, setSession] = useState<{ apiBase: string; shortCode: string } | null>(null);
  const [qrDataUrl, setQrDataUrl] = useState<string>('');
  const [pairing, setPairing] = useState<{ pairingCode: string; pairingUrl: string } | null>(null);
  const [pairedState, setPairedState] = useState<PairingState | null>(null);
  // Set right before a finish-pairing attempt, cleared on success. Lets the
  // UI offer a "Try again" button if the phone-side authentication already
  // succeeded but this device's own finish call failed (network error) —
  // polling has already stopped by that point (the backend's
  // extension_pairing_requests.status is 'completed' and stays that way),
  // so without this the popup would otherwise show a permanent, silent
  // "Waiting for confirmation…" with nothing actually happening.
  const [finishRetryInfo, setFinishRetryInfo] = useState<{
    target: { apiBase: string; shortCode: string };
    pairingCode: string;
    rollNumber: string;
  } | null>(null);
  const pollTimer = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    void bootstrap();
    return () => {
      if (pollTimer.current) clearInterval(pollTimer.current);
    };
  }, []);

  async function bootstrap() {
    await getOrCreateExtensionInstanceId();

    const existing = await getPairingState();
    if (existing) {
      setPairedState(existing);
      setScreen('paired');
      return;
    }

    const consented = await getConsentAcknowledged();
    if (!consented) {
      setScreen('consent');
      return;
    }

    const [activeTab] = await browser.tabs.query({ active: true, currentWindow: true });
    const detected = detectShortCodeFromUrl(activeTab?.url);
    if (!detected) {
      setScreen('no-session');
      return;
    }
    setSession(detected);

    // Resume an in-flight pairing (popup was closed mid-poll) rather than
    // starting a fresh one and orphaning the old pairing_code. Reuses the
    // real `pairingUrl` the backend returned originally — reconstructing it
    // from just shortCode+pairingCode would produce a bare relative path
    // with no scheme/host, which a phone camera can't open as a link.
    const pending = await getPendingPairing();
    if (pending && pending.shortCode === detected.shortCode) {
      setPairing({ pairingCode: pending.pairingCode, pairingUrl: pending.pairingUrl });
      await renderQr(pending.pairingUrl);
      setScreen('pairing');
      beginPolling(detected, pending.pairingCode);
      return;
    }

    setScreen('pairing');
  }

  async function renderQr(text: string) {
    try {
      // `margin: 4` is the ISO/IEC 18004 minimum quiet zone — the previous
      // value of 1 was undersized for a full URL's worth of content (this
      // encodes {publicBaseUrl}/attend/pair/{shortCode}/{pairingCode}, often
      // 60-90+ chars), which pushes the QR to a denser version and makes an
      // already-thin quiet zone a real scan-reliability risk under normal
      // classroom lighting.
      const dataUrl = await QRCode.toDataURL(text, { width: 260, margin: 4, errorCorrectionLevel: 'M' });
      setQrDataUrl(dataUrl);
    } catch {
      setQrDataUrl('');
    }
  }

  async function handleConsent() {
    await setConsentAcknowledged();
    void bootstrap();
  }

  async function handleStartPairing() {
    if (!session) return;
    setError(null);
    try {
      const extensionInstanceId = await getOrCreateExtensionInstanceId();
      const res = await startPairing(session.apiBase, session.shortCode, extensionInstanceId);
      setPairing(res);
      await setPendingPairing({
        apiBase: session.apiBase,
        shortCode: session.shortCode,
        pairingCode: res.pairingCode,
        pairingUrl: res.pairingUrl,
        extensionInstanceId,
      });
      await renderQr(res.pairingUrl);
      beginPolling(session, res.pairingCode);
    } catch {
      setError('Could not start pairing. Check your connection and try again.');
    }
  }

  function beginPolling(target: { apiBase: string; shortCode: string }, pairingCode: string) {
    if (pollTimer.current) clearInterval(pollTimer.current);
    pollTimer.current = setInterval(async () => {
      try {
        const status = await pollPairingStatus(target.apiBase, target.shortCode, pairingCode);
        if (status.status === 'completed' && status.rollNumber) {
          if (pollTimer.current) clearInterval(pollTimer.current);
          await handleFinish(target, pairingCode, status.rollNumber);
        } else if (status.status === 'expired') {
          if (pollTimer.current) clearInterval(pollTimer.current);
          setError('Pairing code expired. Start again.');
          setPairing(null);
          await setPendingPairing(null);
          setScreen('pairing');
        }
      } catch {
        // Transient — keep polling, the next tick will retry.
      }
    }, POLL_INTERVAL_MS);
  }

  async function handleFinish(target: { apiBase: string; shortCode: string }, pairingCode: string, rollNumber: string) {
    setScreen('finishing');
    setFinishRetryInfo({ target, pairingCode, rollNumber });
    try {
      const extensionInstanceId = await getOrCreateExtensionInstanceId();
      const fingerprint = collectDeviceFingerprint();
      const [hash, uaData, multiMonitor, iceCandidateAddresses] = await Promise.all([
        hashFingerprint(fingerprint),
        collectHighEntropyUaData(),
        detectMultiMonitor(),
        collectWebrtcIceCandidates(),
      ]);
      const automationFlags = detectAutomationFlags(fingerprint);

      const finishResult = await finishPairing(
        target.apiBase,
        target.shortCode,
        pairingCode,
        hash,
        uaData?.platform ?? navigator.userAgent,
        fingerprint.platform,
      );

      const newState: PairingState = {
        apiBase: target.apiBase,
        shortCode: target.shortCode,
        sessionId: '',
        rollNumber,
        extensionInstanceId,
        pairedAt: new Date().toISOString(),
        telemetryToken: finishResult.telemetryToken,
        telemetryTokenExpiresAt: finishResult.telemetryTokenExpiresAt,
      };
      await setPairingState(newState);
      await setPendingPairing(null);
      setPairedState(newState);
      setFinishRetryInfo(null);

      void browser.runtime.sendMessage({
        type: 'PAIRING_COMPLETE',
        deviceFingerprint: fingerprint,
        uaData,
        multiMonitor,
        automationFlags,
        iceCandidateAddresses,
      });

      setScreen('paired');
    } catch {
      setError('Pairing completed on your phone, but this device could not finish locking in. Try again.');
      setScreen('pairing');
    }
  }

  async function handleRetryFinish() {
    if (!finishRetryInfo) return;
    const { target, pairingCode, rollNumber } = finishRetryInfo;
    await handleFinish(target, pairingCode, rollNumber);
  }

  async function handleRepair() {
    setPairedState(null);
    await setPendingPairing(null);
    setScreen('pairing');
  }

  if (screen === 'loading') {
    return <div className="panel">Loading…</div>;
  }

  if (screen === 'consent') {
    return (
      <div className="panel">
        <h1>Before you pair this device</h1>
        <p>While monitoring is active for a session, this extension reports:</p>
        <ul>
          <li>Presence &amp; activity — idle/active state, tab and window focus, heartbeat</li>
          <li>Tab &amp; browsing — active tab domain, tab/window counts, tab groups, and <strong>your full browsing history while the extension is active</strong> (not just the class tab)</li>
          <li>Whether your video-call tab is open and focused</li>
          <li>Device signals — hardware/browser fingerprint, used to confirm this is the same device for the whole session</li>
        </ul>
        <p>Data is kept for 90 days, then deleted automatically. Reporting stops the moment the session's monitoring window ends or you uninstall the extension.</p>
        <button onClick={handleConsent}>I understand, continue</button>
      </div>
    );
  }

  if (screen === 'no-session') {
    return (
      <div className="panel">
        <h1>No session detected</h1>
        <p>Open the class session link your mentor shared, then reopen this popup to pair.</p>
      </div>
    );
  }

  if (screen === 'pairing') {
    return (
      <div className="panel">
        <h1>Pair this device</h1>
        {!pairing ? (
          <>
            <p>Start pairing, then scan the QR code with your phone to confirm your identity with your passkey.</p>
            <button onClick={handleStartPairing}>Pair this device</button>
          </>
        ) : (
          <>
            <p>Scan with your phone:</p>
            {qrDataUrl && <img src={qrDataUrl} alt="Pairing QR code" width={260} height={260} />}
            <p className="muted">Code: {pairing.pairingCode}</p>
            <p className="muted">Waiting for confirmation…</p>
          </>
        )}
        {error && <p className="error">{error}</p>}
        {/* Phone-side authentication already succeeded (backend status is
            'completed') but this device's own finish call failed — polling
            has already stopped, so retrying is the only way forward. */}
        {error && finishRetryInfo && <button onClick={handleRetryFinish}>Try again</button>}
      </div>
    );
  }

  if (screen === 'finishing') {
    return <div className="panel">Locking this device in…</div>;
  }

  if (screen === 'paired' && pairedState) {
    return (
      <div className="panel">
        <h1>Paired ✓</h1>
        <p>
          This device is locked to the current session for <strong>{pairedState.rollNumber}</strong>.
        </p>
        <p className="muted">Monitoring is active. Switching devices? Pair the new one — this lock will be released automatically.</p>
        <button onClick={handleRepair}>Pair a different device</button>
      </div>
    );
  }

  return <div className="panel">Something went wrong.</div>;
}
