import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import { useIsMobile } from '../hooks/useIsMobile';
import MobileDeviceRequired from '../components/MobileDeviceRequired';
import PermissionOnboarding from '../components/PermissionOnboarding';
import HelpDrawer from '../components/HelpDrawer';

/* ponytail: globals loaded via CDN in index.html */
declare const FingerprintJS: { load(): Promise<{ get(): Promise<{ visitorId: string }> }> };

type Step = 'loading' | 'permissions' | 'error' | 'rollInput' | 'webauthnAction' | 'form' | 'success';

/** Detect privacy-focused Android OSes from the user agent string (best-effort) */
const detectPrivacyOs = (): boolean => {
  const ua = navigator.userAgent || '';
  return /GrapheneOS|LineageOS|CalyxOS|DivestOS|microG/i.test(ua);
};

interface SessionInfo { locationName: string; expiresAt: string; }


const toB64url = (buf: ArrayBuffer) =>
  btoa(String.fromCharCode(...new Uint8Array(buf))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const fromB64url = (s: string) =>
  Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0));

const Spinner = () => (
  <div className="flex flex-col items-center py-10">
    <div className="spinner" />
    <p className="text-gray-500 text-sm">Loading session...</p>
  </div>
);

const LockIcon = ({ size = 16 }: { size?: number }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
    <rect width="18" height="11" x="3" y="11" rx="2" ry="2" />
    <path d="M7 11V7a5 5 0 0 1 10 0v4" />
  </svg>
);

const AlertTriangleIcon = ({ size = 16 }: { size?: number }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
    <path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" />
    <line x1="12" y1="9" x2="12" y2="13" />
    <line x1="12" y1="17" x2="12.01" y2="17" />
  </svg>
);

const LoaderIcon = ({ size = 16, className }: { size?: number; className?: string }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={className} style={{ display: 'block' }}>
    <path d="M21 12a9 9 0 1 1-6.219-8.56" />
  </svg>
);

const CheckIcon = ({ size = 16 }: { size?: number }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
    <polyline points="20 6 9 17 4 12" />
  </svg>
);

const RotateCwIcon = ({ size = 14 }: { size?: number }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
    <path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.72 2.78L21 8" />
    <polyline points="21 3 21 8 16 8" />
  </svg>
);

const WrenchIcon = ({ size = 14 }: { size?: number }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block' }}>
    <path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" />
  </svg>
);

const AndroidIcon = ({ size = 14, style }: { size?: number; style?: React.CSSProperties }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block', ...style }}>
    <rect width="14" height="20" x="5" y="2" rx="2" ry="2" />
    <path d="M12 18h.01" />
  </svg>
);

const AppleIcon = ({ size = 14, style }: { size?: number; style?: React.CSSProperties }) => (
  <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ display: 'block', ...style }}>
    <path d="M12 20.94c1.88-2.2 4-2.2 4-6 0-3.5-2.5-6-5-6s-5 2.5-5 6c0 3.8 2.12 3.8 4 6Z" />
    <path d="M12 8.94c-.5-1.5 0-3 1.5-3.5a3.5 3.5 0 0 1-1.5 3.5Z" />
  </svg>
);

export default function StudentScan() {
  const { shortCode } = useParams<{ shortCode: string }>();
  const API = window.location.origin;

  const [step, setStep] = useState<Step>('loading');
  const [errMsg, setErrMsg] = useState('');
  const [flashMsg, setFlashMsg] = useState<{ text: React.ReactNode; ok: boolean } | null>(null);
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [devBypassEnabled, setDevBypassEnabled] = useState(false);
  const isMobile = useIsMobile();



  // Roll input step
  const [rollInput, setRollInput] = useState('');
  const [isEnrolled, setIsEnrolled] = useState(false);
  const [isSuspended, setIsSuspended] = useState(false);
  const [webauthnSupported] = useState(() => typeof window.PublicKeyCredential !== 'undefined');

  // Form step
  const [studentName, setStudentName] = useState('');
  const [captchaId, setCaptchaId] = useState('');
  const [captchaSvg, setCaptchaSvg] = useState('');
  const [captchaAnswer, setCaptchaAnswer] = useState('');
  const [locStatus, setLocStatus] = useState<'pending' | 'ok' | 'denied' | 'blocked' | 'retrying'>('pending');
  const [locErrMsg, setLocErrMsg] = useState('');
  const [locData, setLocData] = useState<{ latitude: number; longitude: number } | null>(null);
  const [photoTaken, setPhotoTaken] = useState(false);
  const [webauthnVerified, setWebauthnVerified] = useState(false);
  const [verifyMethod, setVerifyMethod] = useState<React.ReactNode>('');
  const [submitting, setSubmitting] = useState(false);
  const [usedDevBypassCamera, setUsedDevBypassCamera] = useState(false);
  const [usedDevBypassGps, setUsedDevBypassGps] = useState(false);

  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const photoDataRef = useRef('');
  const streamRef = useRef<MediaStream | null>(null);
  const credentialRef = useRef<object | null>(null);
  const rollRef = useRef('');
  const fingerRef = useRef('unknown');
  const conditionalUiAbortRef = useRef<AbortController | null>(null);
  const captchaContainerRef = useRef<HTMLDivElement>(null);

  const flash = (text: React.ReactNode, ok = false) => {
    setFlashMsg({ text, ok });
    setTimeout(() => setFlashMsg(null), 5000);
  };

  const loadCaptcha = useCallback(async () => {
    try {
      const res = await fetch(`${API}/s/${shortCode}/captcha`);
      const data = await res.json();
      setCaptchaId(data.captchaId);
      setCaptchaAnswer('');
      setCaptchaSvg(data.captchaSvg);
    } catch { /* ignore */ }
  }, [shortCode, API]);


  // Ref to store devBypassEnabled for use in handleAcknowledge after session loads
  const devBypassEnabledRef = useRef(false);
  const initAbortRef = useRef<AbortController | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    initAbortRef.current = ac;
    (async () => {
      try {
        const res = await fetch(`${API}/s/${shortCode}/session`, { signal: ac.signal });
        if (!res.ok) throw new Error('Session not found or inactive');
        const data = await res.json();
        if (ac.signal.aborted) return;
        setSession({ locationName: data.session.locationName, expiresAt: data.session.expiresAt });
        const bypass = !!data.devBypassEnabled;
        setDevBypassEnabled(bypass);
        devBypassEnabledRef.current = bypass;
        // Show onboarding screen — camera/captcha/fingerprint deferred to handleAcknowledge
        setStep('permissions');
      } catch (err) {
        if (!ac.signal.aborted) {
          setErrMsg((err as Error).message);
          setStep('error');
        }
      }
    })();
    return () => { 
      ac.abort();
      streamRef.current?.getTracks().forEach(t => t.stop()); 
    };
  }, [shortCode, API, loadCaptcha]);

  /** Initialize camera stream. Extracted as useCallback so handleAcknowledge can depend on it. */
  const initCamera = useCallback(async (signal?: AbortSignal, bypassFlag?: boolean) => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'user' }, audio: false });
      if (signal?.aborted) {
        stream.getTracks().forEach(t => t.stop());
        return;
      }
      streamRef.current = stream;
      if (videoRef.current) { videoRef.current.srcObject = stream; videoRef.current.play().catch(() => {}); }
    } catch (err) {
      const isBypassed = bypassFlag !== undefined ? bypassFlag : devBypassEnabledRef.current;
      if (isBypassed) {
        flash('Camera error, but DEV bypass is available.', true);
        return;
      }
      const e = err as { name: string; message: string };
      const msgs: Record<string, string> = {
        NotAllowedError: 'Camera permission denied. Enable camera in browser settings and refresh.',
        NotFoundError: 'No camera found on this device.',
        NotReadableError: 'Camera is in use by another app. Close it and refresh.',
      };
      setErrMsg(msgs[e.name] ?? `Camera error: ${e.message}`);
      setStep('error');
    }
  }, []);

  /** Called when student taps "I Acknowledge & Continue" on the onboarding screen */
  const handleAcknowledge = useCallback(async () => {
    setStep('rollInput');
    // Now trigger camera, captcha, and fingerprint — student is prepared
    const ac = initAbortRef.current ?? new AbortController();
    await initCamera(ac.signal, devBypassEnabledRef.current);
    await loadCaptcha();
    try {
      const fp = await FingerprintJS.load();
      const result = await fp.get();
      if (!ac.signal.aborted) fingerRef.current = result.visitorId;
    } catch { /* ignore */ }
  }, [loadCaptcha, initCamera]);

  useEffect(() => {
    if (step === 'rollInput') {
      startConditionalUI();
    }
    return () => {
      if (conditionalUiAbortRef.current) {
        conditionalUiAbortRef.current.abort();
      }
    };
  }, [step]);


  // Attach the (pre-acquired) camera stream once the form pane's <video> mounts.
  useEffect(() => {
    if (step === 'form' && streamRef.current && videoRef.current && !videoRef.current.srcObject) {
      videoRef.current.srcObject = streamRef.current;
      videoRef.current.play().catch(() => {});
    }
  }, [step]);

  useEffect(() => {
    if (step === 'form' && captchaSvg && captchaContainerRef.current) {
      const parser = new DOMParser();
      const doc = parser.parseFromString(captchaSvg, 'image/svg+xml');
      const svg = doc.documentElement;
      svg.removeAttribute('width'); svg.removeAttribute('height');
      Object.assign(svg.style, { height: '40px', width: 'auto', display: 'block' });
      captchaContainerRef.current.innerHTML = '';
      captchaContainerRef.current.appendChild(svg);
    }
  }, [step, captchaSvg]);

  const getLocation = useCallback(() => {
    if (!navigator.geolocation) {
      setLocStatus('blocked');
      setLocErrMsg('Geolocation is not supported by this browser.');
      return;
    }
    setLocStatus('pending');
    setLocErrMsg('');

    const onSuccess = (pos: GeolocationPosition) => {
      setLocData({ latitude: pos.coords.latitude, longitude: pos.coords.longitude });
      setLocStatus('ok');
      if (pos.coords.accuracy > 500) {
        flash(<span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}><AlertTriangleIcon size={14} /> Weak signal (±{Math.round(pos.coords.accuracy)}m) — location recorded.</span>, true);
      }
    };

    const onFinalFail = (err: GeolocationPositionError) => {
      if (err.code === 1) {
        setLocStatus('blocked');
        setLocErrMsg('Permission denied. Tap the lock icon in your browser URL bar and allow Location.');
      } else {
        setLocStatus('denied');
        setLocErrMsg(
          err.code === 2
            ? 'No location signal. Enable Wi-Fi or move near a window, then tap Retry.'
            : 'Location timed out. Move near a window, then tap Retry.'
        );
      }
    };

    // Pass 1: high-accuracy GPS (8s). Fails indoors → pass 2 network fallback.
    navigator.geolocation.getCurrentPosition(onSuccess, (err) => {
      if (err.code === 1) { onFinalFail(err); return; }
      // Pass 2: Wi-Fi / cell-based — fast indoors, works without GPS lock
      setLocStatus('retrying');
      navigator.geolocation.getCurrentPosition(onSuccess, onFinalFail, {
        enableHighAccuracy: false,
        timeout: 10000,
        maximumAge: 60000,
      });
    }, { enableHighAccuracy: true, timeout: 8000, maximumAge: 0 });
  }, []);

  // Auto-trigger location + fresh captcha the moment the form step activates
  useEffect(() => {
    if (step === 'form') {
      getLocation();
      loadCaptcha();
    }
  }, [step]);

  // Re-check location when user returns from phone Settings (covers iOS Safari + Android)
  useEffect(() => {
    if (step !== 'form' || locStatus === 'ok') return;
    const onVisible = () => {
      if (document.visibilityState === 'visible') getLocation();
    };
    document.addEventListener('visibilitychange', onVisible);
    return () => document.removeEventListener('visibilitychange', onVisible);
  }, [step, locStatus, getLocation]);

  // Permissions API onchange — auto-retry when OS grants permission (Chrome Android)
  useEffect(() => {
    if (step !== 'form' || locStatus === 'ok') return;
    if (!navigator.permissions) return;
    let permStatus: PermissionStatus | null = null;
    navigator.permissions
      .query({ name: 'geolocation' as PermissionName })
      .then((status) => {
        permStatus = status;
        status.onchange = () => {
          if (status.state === 'granted') getLocation();
        };
      })
      .catch(() => {});
    return () => { if (permStatus) permStatus.onchange = null; };
  }, [step, locStatus, getLocation]);

  const handleDevBypassGps = () => {
    setUsedDevBypassGps(true);
    setLocData({ latitude: 0, longitude: 0 });
    setLocStatus('ok');
    flash('Injected mock location for DEV', true);
  };

  const handleCheckStatus = async () => {
    const roll = rollInput.trim().toUpperCase();
    if (!roll) { flash('Please enter your roll number'); return; }
    if (!/^[A-Z0-9]{3,20}$/.test(roll)) { flash('Invalid roll number format'); return; }
    
    // Cancel any pending conditional UI request before moving to manual flow
    if (conditionalUiAbortRef.current) {
      conditionalUiAbortRef.current.abort();
      conditionalUiAbortRef.current = null;
    }

    rollRef.current = roll;
    try {
      const res = await fetch(`${API}/s/${shortCode}/webauthn/status/${roll}`);
      const data = await res.json();
      if (data.alreadySubmitted) { flash(data.message || 'Attendance already submitted', true); return; }
      setIsEnrolled(data.enrolled);
      setIsSuspended(data.suspended);
      setStep('webauthnAction');
    } catch { flash('Failed to check status. Try again.'); }
  };

  const handleFallback = () => {
    const roll = rollInput.trim().toUpperCase();
    if (!roll) { flash('Enter your roll number first', true); return; }
    rollRef.current = roll;
    setWebauthnVerified(false);
    setVerifyMethod(<span style={{ display: 'inline-flex', alignItems: 'center', gap: '4px' }}><AlertTriangleIcon size={12} /> Manual verification required</span>);
    setLocStatus('pending');
    setLocErrMsg('');
    setStep('form');
  };

  const handleDevBypassWebAuthn = () => {
    const roll = rollInput.trim().toUpperCase();
    if (!roll) { flash('Enter your roll number first', true); return; }
    rollRef.current = roll;
    setWebauthnVerified(true);
    setVerifyMethod('Bypassed (DEV)');
    flash('Identity bypassed in DEV mode!', true);
    setLocStatus('pending');
    setLocErrMsg('');
    setStep('form');
  };

  const register = async () => {
    const name = studentName.trim();
    if (!name || name.length < 2) {
      flash('Valid name required for registration.');
      return;
    }
    try {
      flash('Starting registration...');
      const startRes = await fetch(`${API}/s/${shortCode}/webauthn/register/start`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rollNumber: rollRef.current, studentName: name }),
      });
      if (!startRes.ok) { const e = await startRes.json(); throw new Error(e.message); }
      const opts = await startRes.json();
      opts.challenge = fromB64url(opts.challenge);
      opts.user.id = fromB64url(opts.user.id);

      const credential = await navigator.credentials.create({ publicKey: opts }) as PublicKeyCredential;
      const resp = credential.response as AuthenticatorAttestationResponse;

      flash('Verifying registration...');
      const finishRes = await fetch(`${API}/s/${shortCode}/webauthn/register/finish`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          rollNumber: rollRef.current,
          credential: {
            id: credential.id,
            rawId: toB64url(credential.rawId),
            response: { attestationObject: toB64url(resp.attestationObject), clientDataJSON: toB64url(resp.clientDataJSON) },
            type: credential.type,
            clientExtensionResults: credential.getClientExtensionResults ? credential.getClientExtensionResults() : {},
            authenticatorAttachment: credential.authenticatorAttachment,
          },
        }),
      });
      if (!finishRes.ok) { const e = await finishRes.json(); throw new Error(e.message); }
      const result = await finishRes.json();
      setStudentName(name);
      setWebauthnVerified(true);
      setVerifyMethod('Device: ' + (result.credentialId?.substring(0, 8) || 'Enrolled'));
      flash('Device enrolled!', true);
      setLocStatus('pending');
      setLocErrMsg('');
      setStep('form');
    } catch (err) {
      flash('Registration failed: ' + (err as Error).message);
    }
  };

  const authenticate = async () => {
    try {
      flash('Starting authentication...');
      const startRes = await fetch(`${API}/s/${shortCode}/webauthn/authenticate/start`, {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ rollNumber: rollRef.current }),
      });
      if (!startRes.ok) { const e = await startRes.json(); throw new Error(e.message); }
      const opts = await startRes.json();
      opts.challenge = fromB64url(opts.challenge);
      if (opts.allowCredentials) {
        opts.allowCredentials = opts.allowCredentials.map((c: { id: string; [key: string]: unknown }) => ({ ...c, id: fromB64url(c.id) }));
      }

      const assertion = await navigator.credentials.get({ publicKey: opts }) as PublicKeyCredential;
      const resp = assertion.response as AuthenticatorAssertionResponse;

      credentialRef.current = {
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
      };

      setWebauthnVerified(true);
      setVerifyMethod('Biometric verified');
      flash('Identity verified!', true);
      setLocStatus('pending');
      setLocErrMsg('');
      setStep('form');
    } catch (err) {
      const e = err as { name?: string; message?: string };
      if (e.name === 'NotAllowedError') flash('Authentication cancelled. Please try again.');
      else flash('Authentication failed: ' + e.message);
    }
  };

  const startConditionalUI = async () => {
    try {
      if (!window.PublicKeyCredential?.isConditionalMediationAvailable) return;
      const isAvailable = await PublicKeyCredential.isConditionalMediationAvailable();
      if (!isAvailable) return;
      
      const startRes = await fetch(`${API}/s/${shortCode}/webauthn/authenticate/conditional`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
      });
      if (!startRes.ok) return;
      const opts = await startRes.json();
      opts.challenge = fromB64url(opts.challenge);
      
      const abortController = new AbortController();
      conditionalUiAbortRef.current = abortController;
      
      const assertion = await navigator.credentials.get({
        publicKey: opts,
        mediation: 'conditional',
        signal: abortController.signal,
      }) as PublicKeyCredential;
      
      const resp = assertion.response as AuthenticatorAssertionResponse;
      let userHandle = null;
      if (resp.userHandle) {
        const decoder = new TextDecoder();
        userHandle = decoder.decode(resp.userHandle);
      }
      if (!userHandle) throw new Error('User identification not available from passkey');
      
      rollRef.current = userHandle;
      setRollInput(userHandle);
      
      credentialRef.current = {
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
      };
      
      setWebauthnVerified(true);
      setVerifyMethod('Passkey verified');
      flash('Identity verified with passkey!', true);
      setLocStatus('pending');
      setLocErrMsg('');
      setStep('form');
    } catch (err) {
      const e = err as { name?: string; message?: string };
      // Ignore background errors for conditional UI
      // If it fails, the user simply proceeds with the manual fallback flow.
      if (e.name !== 'AbortError' && e.name !== 'NotAllowedError') {
        // Silently swallow other errors
      }
    }
  };

  const handleCapture = async () => {
    if (photoTaken) {
      // Retake
      photoDataRef.current = '';
      setPhotoTaken(false);
      setUsedDevBypassCamera(false);
      if (videoRef.current && canvasRef.current) {
        videoRef.current.style.display = 'block';
        canvasRef.current.style.display = 'none';
      }
      return;
    }
    const video = videoRef.current;
    const canvas = canvasRef.current;
    if (!video || !canvas) return;
    canvas.width = video.videoWidth || 640;
    canvas.height = video.videoHeight || 480;
    canvas.getContext('2d')?.drawImage(video, 0, 0);
    photoDataRef.current = canvas.toDataURL('image/jpeg', 0.75);
    canvas.style.display = 'block';
    video.style.display = 'none';
    setPhotoTaken(true);
  };

  const handleDevBypassCamera = () => {
    setUsedDevBypassCamera(true);
    photoDataRef.current = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
    if (videoRef.current) videoRef.current.style.display = 'none';
    setPhotoTaken(true);
  };

  const canSubmit = studentName.trim() && captchaAnswer.trim() && photoTaken && locData;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!canSubmit) return;
    setSubmitting(true);
    try {
      const endpoint = credentialRef.current
        ? `${API}/s/${shortCode}/webauthn/authenticate/finish`
        : `${API}/s/${shortCode}/submit`;

      const body: Record<string, unknown> = {
        studentName: studentName.trim(),
        rollNumber: rollRef.current,
        photo: photoDataRef.current,
        latitude: locData?.latitude,
        longitude: locData?.longitude,
        faceDetected: true,
        captchaAnswer: captchaAnswer.trim(),
        captchaId,
        deviceFingerprint: fingerRef.current,
        webauthnVerified,
        devBypassCamera: usedDevBypassCamera,
        devBypassGps: usedDevBypassGps,
        devBypassWebauthn: devBypassEnabled && !credentialRef.current,
      };
      if (credentialRef.current) body.credential = credentialRef.current;

      const res = await fetch(endpoint, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
      const result = await res.json();
      if (!res.ok) throw new Error(result.message || 'Failed to submit');
      setStep('success');
    } catch (err) {
      flash((err as Error).message);
      setSubmitting(false);
      loadCaptcha();
    }
  };

  const Logo = (
    <div className="attend-pane-logo">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
      </svg>
    </div>
  );


  const CameraPane = (
    <div className="attend-camera-pane">
      <div className="attend-pane-inner">
        <div className="attend-pane-head">
          {Logo}
          <div>
            <div className="attend-pane-title">Mark Attendance</div>
            <div className="attend-pane-sub">{session?.locationName}</div>
          </div>
        </div>
        <div className={`attend-camera${photoTaken ? ' captured' : ''}`}>
          <video ref={videoRef} autoPlay playsInline muted />
          <canvas ref={canvasRef} style={{ display: 'none' }} />
          <div className="attend-camera-guide" />
          <span className="attend-camera-badge"><span className="dot" />{photoTaken ? 'Captured' : 'Live'}</span>
        </div>
        <button type="button" className={`attend-capture-btn${photoTaken ? ' retake' : ''}`} onClick={handleCapture}>
          {photoTaken ? '↺  Retake photo' : '◉  Capture photo'}
        </button>
        {devBypassEnabled && !photoTaken && (
          <button type="button" className="attend-btn-ghost" onClick={handleDevBypassCamera} style={{ marginTop: 8, color: '#667085', borderColor: '#d0d5dd' }}>
            🛠️ Use Mock Photo (DEV)
          </button>
        )}
      </div>
    </div>
  );

  const isPrivacyOs = detectPrivacyOs();

  if (!isMobile && !devBypassEnabled && step !== 'loading' && step !== 'error' && step !== 'permissions') {
    return <MobileDeviceRequired />;
  }

  return (
    <>
    {/* Floating help button — visible on all steps except loading and success */}
    <HelpDrawer currentStep={step} />

    {/* ── Permission Onboarding Screen ── */}
    {step === 'permissions' && session && (
      <PermissionOnboarding
        locationName={session.locationName}
        onAcknowledge={handleAcknowledge}
        isPrivacyOs={isPrivacyOs}
      />
    )}
    {step !== 'form' && step !== 'permissions' && (
    <div className="attend-page">
      {step === 'loading' && (
        <div className="attend-card"><div className="attend-center"><Spinner /></div></div>
      )}

      {step === 'error' && (
        <div className="attend-card">
          <div style={{ padding: 28 }}>
            <div className="attend-status err" style={{ marginBottom: 0 }}><span>{errMsg}</span></div>
            <p style={{ fontSize: 13, color: '#667085', marginTop: 12, textAlign: 'center' }}>Please contact your instructor.</p>
          </div>
        </div>
      )}

      {(step === 'rollInput' || step === 'webauthnAction' || step === 'success') && (
        <div className="attend-stack">
          {flashMsg && <div className={`attend-flash ${flashMsg.ok ? 'ok' : 'err'}`}>{flashMsg.text}</div>}
          <div className="attend-card">
            <div className="attend-form-pane">
              {step === 'rollInput' && (
                <>
                  <h2>Mark Attendance</h2>
                  <p className="sub" style={{ marginBottom: 20 }}>Enter your roll number to begin</p>
                  {!webauthnSupported && (
                    <div className="attend-status warn">
                      <span>
                        <strong>Biometric not supported</strong>
                        <div className="detail">You can still submit, but it will be flagged. </div>
                        <button onClick={handleFallback} className="attend-btn-ghost" style={{ width: 'auto', padding: 0, marginTop: 4 }}>Continue without biometric</button>
                      </span>
                    </div>
                  )}
                  <div className="attend-field">
                    <label>Roll Number</label>
                    <input className="attend-input" placeholder="e.g. 21CS042" value={rollInput}
                      onChange={(e) => setRollInput(e.target.value.toUpperCase())}
                      onKeyDown={(e) => e.key === 'Enter' && handleCheckStatus()}
                      autoComplete="username webauthn"
                      style={{ textTransform: 'uppercase' }} />
                  </div>
                  <button className="attend-btn" onClick={handleCheckStatus} style={{ marginTop: 16 }}>Check Status</button>
                </>
              )}

              {step === 'webauthnAction' && (
                <>
                  <h2>Verify Identity</h2>
                  <div className={`attend-status ${isSuspended ? 'err' : 'info'}`} style={{ marginTop: 18 }}>
                    <span>{isSuspended
                      ? 'Your credential has been suspended. Contact admin.'
                      : isEnrolled
                      ? 'Your device is enrolled. Verify your identity to continue.'
                      : 'No device enrolled. Register your biometric to continue.'}</span>
                  </div>
                  {isEnrolled && !isSuspended && (
                    <button className="attend-btn" onClick={authenticate}>
                      🔑 Verify Identity (Passkey / Phone)
                    </button>
                  )}
                  {(!isEnrolled || isSuspended) && (
                    <>
                      <div className="attend-field" style={{ marginTop: 16, marginBottom: 16 }}>
                        <label>Full Name</label>
                        <input className="attend-input" placeholder="Enter your full name" value={studentName} onChange={(e) => setStudentName(e.target.value)} disabled={isSuspended} />
                      </div>
                      <button className="attend-btn" onClick={register} disabled={isSuspended || studentName.trim().length < 2}>
                        📱 Enroll Device
                      </button>
                    </>
                  )}
                  {devBypassEnabled && (
                    <button className="attend-btn-ghost" style={{ marginTop: 16, border: '1px dashed #667085', color: '#667085' }} onClick={handleDevBypassWebAuthn}>
                      🛠️ Bypass Biometrics (DEV)
                    </button>
                  )}
                  <button className="attend-btn-ghost" onClick={() => setStep('rollInput')} style={{ marginTop: 8 }}>← Back</button>
                </>
              )}

              {step === 'success' && (
                <div className="attend-center">
                  <div style={{ marginBottom: '16px' }}>
                    <svg
                      width="72"
                      height="72"
                      viewBox="0 0 24 24"
                      fill="none"
                      stroke="#027a48"
                      strokeWidth="2.5"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      style={{ 
                        animation: 'pulse 2s cubic-bezier(0.4, 0, 0.6, 1) infinite',
                        filter: 'drop-shadow(0px 4px 6px rgba(2, 122, 72, 0.2))' 
                      }}
                    >
                      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14" />
                      <polyline points="22 4 12 14.01 9 11.01" />
                    </svg>
                  </div>
                  <style>
                    {`
                      @keyframes pulse {
                        0%, 100% { opacity: 1; transform: scale(1); }
                        50% { opacity: 0.8; transform: scale(1.05); }
                      }
                    `}
                  </style>
                  <h3 style={{ fontSize: 20, fontWeight: 700, color: '#027a48', marginBottom: '8px' }}>Attendance Recorded!</h3>
                  <p style={{ fontSize: 14, color: '#667085' }}>Your attendance has been successfully submitted.</p>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
    )}

      {step === 'form' && (
        <div className="attend-fullscreen">
          {CameraPane}
          <div className="attend-form-pane">
            <div className="attend-pane-inner">
              {flashMsg && <div className={`attend-flash ${flashMsg.ok ? 'ok' : 'err'}`} style={{ boxShadow: 'none' }}>{flashMsg.text}</div>}
              <div>
                <h2>Confirm details</h2>
                <p className="sub">Fill in your details to mark attendance</p>
              </div>

              <div className={`attend-status-card ${locStatus === 'ok' ? 'ok' : (locStatus === 'blocked' || locStatus === 'denied') ? 'err' : 'pending'}`}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '12px',
                  padding: '16px',
                  borderRadius: '12px',
                  border: '1px solid',
                  fontSize: '13px',
                  lineHeight: '1.5',
                  transition: 'all 0.2s ease',
                  background: locStatus === 'ok' ? 'var(--color-success-bg)' : (locStatus === 'blocked' || locStatus === 'denied') ? 'var(--color-danger-bg)' : 'var(--color-bg-subtle)',
                  borderColor: locStatus === 'ok' ? 'rgba(2, 122, 72, 0.15)' : (locStatus === 'blocked' || locStatus === 'denied') ? 'rgba(180, 35, 24, 0.15)' : 'var(--color-border)',
                  color: locStatus === 'ok' ? 'var(--color-success)' : (locStatus === 'blocked' || locStatus === 'denied') ? 'var(--color-danger)' : 'var(--color-muted)'
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontWeight: 600 }}>
                  {locStatus === 'ok' && <CheckIcon size={18} />}
                  {locStatus === 'blocked' && <LockIcon size={18} />}
                  {locStatus === 'denied' && <AlertTriangleIcon size={18} />}
                  {locStatus === 'retrying' && <LoaderIcon size={18} className="animate-spin" />}
                  {locStatus === 'pending' && <LoaderIcon size={18} className="animate-spin" />}
                  
                  <span>
                    {locStatus === 'ok'       && 'Location acquired'}
                    {locStatus === 'blocked'  && 'Location permission blocked'}
                    {locStatus === 'denied'   && 'Location unavailable'}
                    {locStatus === 'retrying' && 'Retrying with network location…'}
                    {locStatus === 'pending'  && 'Getting location…'}
                  </span>
                </div>

                {locStatus === 'ok' && (
                  <div style={{ fontSize: '12px', opacity: 0.8, marginLeft: '26px' }}>
                    {locData?.latitude.toFixed(5)}, {locData?.longitude.toFixed(5)}
                  </div>
                )}

                {(locStatus === 'denied' || locStatus === 'blocked') && locErrMsg && (
                  <div style={{ fontSize: '12px', opacity: 0.8, marginLeft: '26px' }}>
                    {locErrMsg}
                  </div>
                )}

                {locStatus === 'blocked' && (
                  <div style={{ 
                    display: 'flex', 
                    flexDirection: 'column', 
                    gap: '8px', 
                    fontSize: '12px', 
                    marginLeft: '26px', 
                    marginTop: '4px',
                    padding: '10px 12px',
                    borderRadius: '8px',
                    background: 'rgba(180, 35, 24, 0.04)',
                    border: '1px solid rgba(180, 35, 24, 0.08)'
                  }}>
                    <div style={{ display: 'flex', alignItems: 'flex-start', gap: '8px' }}>
                      <AndroidIcon size={14} style={{ marginTop: '2px', flexShrink: 0 }} />
                      <div>
                        <strong>Android (Chrome):</strong> Settings → Apps → Chrome → Permissions → Location → Allow
                      </div>
                    </div>
                    <div style={{ display: 'flex', alignItems: 'flex-start', gap: '8px' }}>
                      <AppleIcon size={14} style={{ marginTop: '2px', flexShrink: 0 }} />
                      <div>
                        <strong>iOS (Safari):</strong> Settings → Privacy → Location Services → Safari → While Using
                      </div>
                    </div>
                  </div>
                )}

                {(locStatus === 'denied' || locStatus === 'blocked') && (
                  <div style={{ display: 'flex', gap: '8px', marginLeft: '26px', marginTop: '4px' }}>
                    <button type="button" onClick={getLocation}
                      style={{
                        display: 'inline-flex',
                        alignItems: 'center',
                        gap: '6px',
                        padding: '6px 12px',
                        borderRadius: '6px',
                        fontSize: '12px',
                        fontWeight: 600,
                        cursor: 'pointer',
                        border: '1px solid var(--color-danger)',
                        background: 'transparent',
                        color: 'var(--color-danger)',
                        transition: 'all 0.15s ease'
                      }}
                      onMouseEnter={(e) => {
                        e.currentTarget.style.background = 'rgba(180, 35, 24, 0.06)';
                      }}
                      onMouseLeave={(e) => {
                        e.currentTarget.style.background = 'transparent';
                      }}
                    >
                      <RotateCwIcon size={12} />
                      Retry Location
                    </button>

                    {devBypassEnabled && (
                      <button type="button" onClick={handleDevBypassGps}
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: '6px',
                          padding: '6px 12px',
                          borderRadius: '6px',
                          fontSize: '12px',
                          fontWeight: 600,
                          cursor: 'pointer',
                          border: '1px solid rgba(180, 35, 24, 0.4)',
                          background: 'transparent',
                          color: 'var(--color-danger)',
                          opacity: 0.8,
                          transition: 'all 0.15s ease'
                        }}
                        onMouseEnter={(e) => {
                          e.currentTarget.style.background = 'rgba(180, 35, 24, 0.06)';
                          e.currentTarget.style.opacity = '1';
                        }}
                        onMouseLeave={(e) => {
                          e.currentTarget.style.background = 'transparent';
                          e.currentTarget.style.opacity = '0.8';
                        }}
                      >
                        <WrenchIcon size={12} />
                        Mock Location
                      </button>
                    )}
                  </div>
                )}
              </div>

              <div className={`attend-status ${webauthnVerified ? 'info' : 'warn'}`}>
                <span><strong>{webauthnVerified ? 'Biometric verified' : 'Fallback mode'}</strong><div className="detail">{verifyMethod}</div></span>
              </div>

              <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                <div className="attend-field">
                  <label>Full Name</label>
                  <input className="attend-input" placeholder="Enter your full name" value={studentName} onChange={(e) => setStudentName(e.target.value)} required />
                </div>

                <div className="attend-field">
                  <label>Captcha Verification</label>
                  <div className="attend-captcha">
                    <div ref={captchaContainerRef} className="attend-captcha-svg" />
                    <button type="button" onClick={loadCaptcha} className="attend-icon-btn" aria-label="Refresh captcha">↻</button>
                  </div>
                  <input className="attend-input" placeholder="Enter the code shown" value={captchaAnswer} onChange={(e) => setCaptchaAnswer(e.target.value)} autoComplete="off" required />
                </div>

                <button type="submit" className="attend-btn" disabled={!canSubmit || submitting}>
                  {submitting ? 'Submitting…' : 'Submit Attendance'}
                </button>
              </form>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
