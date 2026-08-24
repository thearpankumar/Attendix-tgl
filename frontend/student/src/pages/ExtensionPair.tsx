import { useEffect, useRef, useState } from 'react';
import { useParams } from 'react-router';
import { useMobileVerification } from '../hooks/useIsMobile';
import MobileDeviceRequired from '../components/MobileDeviceRequired';

/**
 * The phone-side half of browser-extension device pairing
 * (backend-rust/src/controllers/extension_pairing.rs). The extension popup
 * on a student's laptop renders a QR code pointing here
 * (`{publicBaseUrl}/attend/pair/{shortCode}/{pairingCode}`); scanning it with
 * a phone lands on this page, which runs the SAME WebAuthn authenticate
 * ceremony `StudentScan.tsx` uses for attendance check-in — just against the
 * pairing-specific finish endpoint instead of attendance submission, so a
 * student who already checked in can still re-pair a replacement laptop.
 */

type Step = 'checkingPairing' | 'rollInput' | 'authenticating' | 'success' | 'error';

const toB64url = (buf: ArrayBuffer) =>
  btoa(String.fromCharCode(...new Uint8Array(buf))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const fromB64url = (s: string) =>
  Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/')), (c) => c.charCodeAt(0));

interface WebAuthnStatusResponse {
  enrolled: boolean;
  suspended: boolean;
}

interface AuthenticationOptionsResponse {
  challenge: string;
  timeout?: number;
  rpId?: string;
  allowCredentials?: { id: string; type: 'public-key'; transports?: AuthenticatorTransport[] }[];
  userVerification?: UserVerificationRequirement;
  /** Correlates this ceremony with its own pending challenge row server-side
   *  — must be sent back unchanged in the `finish` request body. See backend
   *  `take_pending_challenge`'s doc comment for why a "most recently
   *  created" guess could consume the wrong ceremony's challenge under
   *  concurrent scans (e.g. two students pairing/authenticating on the same
   *  session link near-simultaneously). */
  challengeId: string;
}

export default function ExtensionPair() {
  const { shortCode, pairingCode } = useParams<{ shortCode: string; pairingCode: string }>();
  const { isMobile, isEmulation, inconsistencies, checking } = useMobileVerification();

  const [step, setStep] = useState<Step>('checkingPairing');
  const [error, setError] = useState<string | null>(null);
  const [rollInput, setRollInput] = useState('');
  const [pairedRollNumber, setPairedRollNumber] = useState('');
  const rollRef = useRef('');

  const API = window.location.origin;

  useEffect(() => {
    // Only run once on mount — re-running on every render would spam the
    // status endpoint and fight with the WebAuthn ceremony's own state.
    void checkPairingStatus();
  }, []);

  async function checkPairingStatus() {
    if (!shortCode || !pairingCode) {
      setError('This pairing link is missing information. Go back to your laptop and start again.');
      setStep('error');
      return;
    }
    try {
      const res = await fetch(`${API}/s/${shortCode}/extension/pair/status/${pairingCode}`);
      if (!res.ok) {
        setError('This pairing code was not found. Go back to your laptop and start again.');
        setStep('error');
        return;
      }
      const data = await res.json();
      if (data.status === 'expired') {
        setError('This pairing code has expired. Go back to your laptop and start again.');
        setStep('error');
        return;
      }
      if (data.status === 'completed') {
        setError('This pairing code has already been used. Go back to your laptop and start a new pairing.');
        setStep('error');
        return;
      }
      setStep('rollInput');
    } catch {
      setError('Could not reach the server. Check your connection and reload this page.');
      setStep('error');
    }
  }

  async function handleSubmitRoll() {
    const roll = rollInput.trim().toUpperCase();
    if (!roll) {
      setError('Please enter your roll number.');
      return;
    }
    if (!/^[A-Z0-9]{3,20}$/.test(roll)) {
      setError('Invalid roll number format.');
      return;
    }
    rollRef.current = roll;
    setError(null);

    try {
      const statusRes = await fetch(`${API}/s/${shortCode}/webauthn/status/${roll}`);
      if (!statusRes.ok) {
        setError('Could not verify your roll number. Try again.');
        return;
      }
      const status: WebAuthnStatusResponse = await statusRes.json();
      if (!status.enrolled) {
        setError('No passkey found for this roll number. Open this session\'s link and register your device first, then rescan this code.');
        return;
      }
      if (status.suspended) {
        setError('Your passkey has been suspended. Contact your admin.');
        return;
      }
      await authenticateAndFinish(roll);
    } catch {
      setError('Could not verify your roll number. Try again.');
    }
  }

  async function authenticateAndFinish(roll: string) {
    if (typeof window.PublicKeyCredential === 'undefined') {
      setError("This browser doesn't support passkeys. Try opening this link in a different browser (e.g. Chrome or Safari).");
      return;
    }
    setStep('authenticating');
    try {
      const startRes = await fetch(`${API}/s/${shortCode}/webauthn/authenticate/start`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rollNumber: roll }),
      });
      if (!startRes.ok) {
        const e = await startRes.json();
        throw new Error(e.message);
      }
      const opts: AuthenticationOptionsResponse = await startRes.json();
      // Built explicitly from only the real WebAuthn fields, excluding
      // `challengeId` — it isn't part of the PublicKeyCredentialRequestOptions
      // dictionary the browser expects.
      const publicKey: PublicKeyCredentialRequestOptions = {
        timeout: opts.timeout,
        rpId: opts.rpId,
        userVerification: opts.userVerification,
        challenge: fromB64url(opts.challenge),
        allowCredentials: opts.allowCredentials?.map((c) => ({ ...c, id: fromB64url(c.id) })),
      };

      const assertion = (await navigator.credentials.get({ publicKey })) as PublicKeyCredential;
      const resp = assertion.response as AuthenticatorAssertionResponse;

      const finishRes = await fetch(`${API}/s/${shortCode}/extension/pair/authenticate/finish`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          pairingCode,
          challengeId: opts.challengeId,
          credential: {
            id: assertion.id,
            rawId: toB64url(assertion.rawId),
            response: {
              authenticatorData: toB64url(resp.authenticatorData),
              clientDataJSON: toB64url(resp.clientDataJSON),
              signature: toB64url(resp.signature),
              userHandle: resp.userHandle ? toB64url(resp.userHandle) : null,
            },
            type: assertion.type,
            clientExtensionResults: assertion.getClientExtensionResults ? assertion.getClientExtensionResults() : {},
            authenticatorAttachment: assertion.authenticatorAttachment,
          },
        }),
      });
      if (!finishRes.ok) {
        const e = await finishRes.json();
        throw new Error(e.message);
      }

      setPairedRollNumber(roll);
      setStep('success');
    } catch (err) {
      const e = err as { name?: string; message?: string };
      if (e.name === 'NotAllowedError') {
        setError('Authentication cancelled. Try again.');
        setStep('rollInput');
      } else if (e.message?.toLowerCase().includes('expired')) {
        // The pairing code's 5-minute TTL can run out mid-ceremony (opening
        // the camera, typing a roll number, completing biometrics all take
        // time) — looping back to rollInput would just hit the same
        // "expired" error again on every retry with no way out, since the
        // code itself never becomes valid again.
        setError('This pairing code has expired. Go back to your laptop and start a new pairing.');
        setStep('error');
      } else {
        setError(e.message || 'Authentication failed. Try again.');
        setStep('rollInput');
      }
    }
  }

  if (checking) {
    return (
      <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 flex items-center justify-center">
        <div className="text-center">
          <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-indigo-600 mx-auto mb-4"></div>
          <p className="text-gray-600">Verifying device...</p>
        </div>
      </div>
    );
  }

  if (!isMobile) {
    return <MobileDeviceRequired isEmulation={isEmulation} inconsistencies={inconsistencies} />;
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-gray-50 to-gray-100 flex items-center justify-center px-4">
      <div className="max-w-sm w-full bg-white rounded-2xl shadow p-6 text-center">
        <h1 className="text-lg font-semibold text-gray-900 mb-2">Pair Your Laptop</h1>

        {step === 'checkingPairing' && (
          <div className="flex flex-col items-center py-6">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600 mb-3"></div>
            <p className="text-gray-500 text-sm">Checking pairing code...</p>
          </div>
        )}

        {step === 'rollInput' && (
          <>
            <p className="text-gray-600 text-sm mb-4">
              Confirm it&apos;s you with your passkey to lock this session&apos;s monitoring to your laptop.
            </p>
            <input
              type="text"
              value={rollInput}
              onChange={(e) => setRollInput(e.target.value)}
              placeholder="Roll number"
              className="w-full border border-gray-300 rounded-lg px-3 py-2 mb-3 text-center uppercase"
              autoCapitalize="characters"
              autoComplete="off"
            />
            {error && <p className="text-red-600 text-sm mb-3">{error}</p>}
            <button
              onClick={handleSubmitRoll}
              className="w-full bg-indigo-600 text-white rounded-lg py-2 font-medium"
            >
              Confirm with passkey
            </button>
          </>
        )}

        {step === 'authenticating' && (
          <div className="flex flex-col items-center py-6">
            <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-indigo-600 mb-3"></div>
            <p className="text-gray-500 text-sm">Waiting for your passkey...</p>
          </div>
        )}

        {step === 'success' && (
          <>
            <p className="text-green-600 font-medium mb-2">Device paired ✓</p>
            <p className="text-gray-600 text-sm">
              This laptop is now locked to the session for <strong>{pairedRollNumber}</strong>. You can close this
              page and return to your laptop.
            </p>
          </>
        )}

        {step === 'error' && (
          <p className="text-red-600 text-sm">{error}</p>
        )}
      </div>
    </div>
  );
}
