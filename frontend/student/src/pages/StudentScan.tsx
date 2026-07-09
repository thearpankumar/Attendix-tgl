import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams } from 'react-router-dom';

/* ponytail: globals loaded via CDN in index.html */
declare const FingerprintJS: { load(): Promise<{ get(): Promise<{ visitorId: string }> }> };

type Step = 'loading' | 'error' | 'rollInput' | 'webauthnAction' | 'form' | 'success';

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

export default function StudentScan() {
  const { shortCode } = useParams<{ shortCode: string }>();
  const API = window.location.origin;

  const [step, setStep] = useState<Step>('loading');
  const [errMsg, setErrMsg] = useState('');
  const [flashMsg, setFlashMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [session, setSession] = useState<SessionInfo | null>(null);



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
  const [locStatus, setLocStatus] = useState<'pending' | 'ok' | 'denied'>('pending');
  const [locData, setLocData] = useState<{ latitude: number; longitude: number } | null>(null);
  const [photoTaken, setPhotoTaken] = useState(false);
  const [webauthnVerified, setWebauthnVerified] = useState(false);
  const [verifyMethod, setVerifyMethod] = useState('');
  const [submitting, setSubmitting] = useState(false);

  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const photoDataRef = useRef('');
  const streamRef = useRef<MediaStream | null>(null);
  const credentialRef = useRef<object | null>(null);
  const rollRef = useRef('');
  const fingerRef = useRef('');
  const captchaContainerRef = useRef<HTMLDivElement>(null);

  const flash = (text: string, ok = false) => {
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


  useEffect(() => {
    const ac = new AbortController();
    (async () => {
      try {
        const res = await fetch(`${API}/s/${shortCode}/session`, { signal: ac.signal });
        if (!res.ok) throw new Error('Session not found or inactive');
        const data = await res.json();
        if (ac.signal.aborted) return;
        setSession({ locationName: data.session.locationName, expiresAt: data.session.expiresAt });
        setStep('rollInput');
        await initCamera(ac.signal);
        await loadCaptcha();
        // fingerprint
        try {
          const fp = await FingerprintJS.load();
          const result = await fp.get();
          if (!ac.signal.aborted) fingerRef.current = result.visitorId;
        } catch { /* ignore */ }
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

  const initCamera = async (signal?: AbortSignal) => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'user' }, audio: false });
      if (signal?.aborted) {
        stream.getTracks().forEach(t => t.stop());
        return;
      }
      streamRef.current = stream;
      if (videoRef.current) { videoRef.current.srcObject = stream; videoRef.current.play().catch(() => {}); }
    } catch (err) {
      const e = err as { name: string; message: string };
      const msgs: Record<string, string> = {
        NotAllowedError: 'Camera permission denied. Enable camera in browser settings and refresh.',
        NotFoundError: 'No camera found on this device.',
        NotReadableError: 'Camera is in use by another app. Close it and refresh.',
      };
      setErrMsg(msgs[e.name] ?? `Camera error: ${e.message}`);
      setStep('error');
    }
  };

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
    if (!navigator.geolocation) { setLocStatus('denied'); return; }
    navigator.geolocation.getCurrentPosition(
      (pos) => { 
        if (pos.coords.accuracy > 200) {
          flash('GPS signal is too weak. Please step outside for better accuracy.', true);
          setLocStatus('denied');
          return;
        }
        setLocData({ latitude: pos.coords.latitude, longitude: pos.coords.longitude }); 
        setLocStatus('ok'); 
      },
      () => setLocStatus('denied'),
      { enableHighAccuracy: true, timeout: 10000 }
    );
  }, []);

  const handleCheckStatus = async () => {
    const roll = rollInput.trim().toUpperCase();
    if (!roll) { flash('Please enter your roll number'); return; }
    if (!/^[A-Z0-9]{3,20}$/.test(roll)) { flash('Invalid roll number format'); return; }
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
    setWebauthnVerified(false);
    setVerifyMethod('⚠️ Manual verification required');
    setStep('form');
    getLocation();
    loadCaptcha();
  };

  const register = async () => {
    const name = prompt('Enter your full name for registration:');
    if (!name || name.trim().length < 2) {
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
          },
        }),
      });
      if (!finishRes.ok) { const e = await finishRes.json(); throw new Error(e.message); }
      const result = await finishRes.json();
      setStudentName(name);
      setWebauthnVerified(true);
      setVerifyMethod('Device: ' + (result.credentialId?.substring(0, 8) || 'Enrolled'));
      flash('Device enrolled!', true);
      setStep('form');
      getLocation();
      loadCaptcha();
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
      };

      setWebauthnVerified(true);
      setVerifyMethod('Biometric verified');
      flash('Identity verified!', true);
      setStep('form');
      getLocation();
      loadCaptcha();
    } catch (err) {
      const e = err as { name?: string; message?: string };
      if (e.name === 'NotAllowedError') flash('Authentication cancelled. Please try again.');
      else flash('Authentication failed: ' + e.message);
    }
  };

  const handleCapture = async () => {
    if (photoTaken) {
      // Retake
      photoDataRef.current = '';
      setPhotoTaken(false);
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
      </div>
    </div>
  );

  return (
    <>
    {step !== 'form' && (
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
                      style={{ textTransform: 'uppercase' }} />
                  </div>
                  <button className="attend-btn" onClick={handleCheckStatus}>Check Status</button>
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
                  <button className="attend-btn" onClick={isEnrolled ? authenticate : register} disabled={isSuspended}>
                    {isEnrolled ? '🔐  Verify Identity' : '📱  Enroll Device'}
                  </button>
                  <button className="attend-btn-ghost" onClick={() => setStep('rollInput')}>← Back</button>
                </>
              )}

              {step === 'success' && (
                <div className="attend-center">
                  <div style={{ fontSize: 52 }}>✅</div>
                  <h3 style={{ fontSize: 18, fontWeight: 700, color: '#027a48' }}>Attendance Recorded!</h3>
                  <p style={{ fontSize: 13, color: '#667085' }}>Your attendance has been successfully submitted.</p>
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

              <div className={`attend-status ${locStatus === 'ok' ? 'ok' : locStatus === 'denied' ? 'err' : 'pending'}`}>
                <span>
                  <strong>{locStatus === 'ok' ? 'Location acquired' : locStatus === 'denied' ? 'Location denied' : 'Getting location…'}</strong>
                  {locStatus === 'ok' && <div className="detail">{locData?.latitude.toFixed(5)}, {locData?.longitude.toFixed(5)}</div>}
                  {locStatus === 'denied' && <div className="detail">Attendance may be unverified.</div>}
                </span>
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
