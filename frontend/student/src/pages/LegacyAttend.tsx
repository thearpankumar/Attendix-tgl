import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams } from 'react-router-dom';
import { useIsMobile } from '../hooks/useIsMobile';
import MobileDeviceRequired from '../components/MobileDeviceRequired';

/* ponytail: face-api.js loaded via CDN in index.html */
declare const faceapi: {
  nets: { ssdMobilenetv1: { loadFromUri(uri: string): Promise<void> } };
  SsdMobilenetv1Options: new (opts: { minConfidence: number }) => unknown;
  detectAllFaces(input: HTMLCanvasElement, opts: unknown): Promise<unknown[]>;
};

interface SessionInfo { locationName: string; expiresAt: string; }
interface StorageInfo { provider: string; supportsDirectUpload?: boolean; }
interface Loc { latitude: number; longitude: number; accuracy: number; }
type Step = 'loading' | 'form' | 'success' | 'error';

const GEO_ERRORS: Record<number, { title: string; detail: string }> = {
  1: { title: '⚠ Location Permission Denied', detail: 'Allow location access in browser settings and refresh.' },
  2: { title: '⚠ Location Unavailable', detail: 'GPS signal could not be obtained. Try moving outdoors.' },
  3: { title: '⚠ Location Timeout', detail: 'Location request timed out. Check GPS is enabled and retry.' },
};

let faceModelLoaded = false;
const loadFaceModel = async () => {
  if (faceModelLoaded || typeof faceapi === 'undefined') return;
  await faceapi.nets.ssdMobilenetv1.loadFromUri('/models');
  faceModelLoaded = true;
};
loadFaceModel().catch(() => {});

export default function LegacyAttend() {
  const { shortCode } = useParams<{ shortCode: string }>();
  const API = window.location.origin;

  const [step, setStep] = useState<Step>('loading');
  const [errMsg, setErrMsg] = useState('');
  const [session, setSession] = useState<SessionInfo | null>(null);
  const [storageInfo, setStorageInfo] = useState<StorageInfo | null>(null);
  const [devBypassEnabled, setDevBypassEnabled] = useState(false);
  const isMobile = useIsMobile();

  const [name, setName] = useState('');
  const [roll, setRoll] = useState('');
  const [captchaId, setCaptchaId] = useState('');
  const [captchaSvg, setCaptchaSvg] = useState('');
  const [captchaAnswer, setCaptchaAnswer] = useState('');
  const [loc, setLoc] = useState<Loc | null>(null);
  const [locMsg, setLocMsg] = useState<{ title: string; detail: string; ok: boolean }>({ title: 'Requesting Location...', detail: 'Allow location when your browser prompts.', ok: false });
  const [photoTaken, setPhotoTaken] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [usedDevBypassCamera, setUsedDevBypassCamera] = useState(false);
  const [usedDevBypassGps, setUsedDevBypassGps] = useState(false);
  const [distanceInfo, setDistanceInfo] = useState('');
  const [flashMsg, setFlashMsg] = useState<{ text: string; ok: boolean } | null>(null);

  const flash = (text: string, ok = false) => {
    setFlashMsg({ text, ok });
    setTimeout(() => setFlashMsg(null), 3000);
  };

  const videoRef = useRef<HTMLVideoElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const captchaRef = useRef<HTMLDivElement>(null);
  const photoDataRef = useRef('');
  const faceDetectedRef = useRef(true);
  const streamRef = useRef<MediaStream | null>(null);

  const loadCaptcha = useCallback(async () => {
    try {
      const res = await fetch(`${API}/s/${shortCode}/captcha`);
      const data = await res.json();
      setCaptchaId(data.captchaId);
      setCaptchaAnswer('');
      setCaptchaSvg(data.captchaSvg);
    } catch { /* ignore */ }
  }, [API, shortCode]);

  useEffect(() => {
    if (step === 'form' && captchaSvg && captchaRef.current) {
      const parser = new DOMParser();
      const doc = parser.parseFromString(captchaSvg, 'image/svg+xml');
      const svg = doc.documentElement as unknown as HTMLElement;
      svg.removeAttribute('width'); svg.removeAttribute('height');
      Object.assign(svg.style, { height: '40px', width: 'auto', display: 'block' });
      captchaRef.current.innerHTML = '';
      captchaRef.current.appendChild(svg);
    }
  }, [step, captchaSvg]);

  const getLocation = useCallback(() => {
    if (!navigator.geolocation) {
      setLocMsg({ title: '⚠ Not Supported', detail: 'Geolocation is not supported by this browser.', ok: false });
      return;
    }
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        if (pos.coords.accuracy > 200) {
          flash('GPS signal is too weak.', true);
          setLocMsg({ title: '⚠ Accuracy Too Low', detail: `Accuracy: ±${Math.round(pos.coords.accuracy)}m (> 200m). Please move outside.`, ok: false });
          return;
        }
        const coords = { latitude: pos.coords.latitude, longitude: pos.coords.longitude, accuracy: pos.coords.accuracy };
        setLoc(coords);
        setLocMsg({ title: '✓ Location Detected', detail: `${coords.latitude.toFixed(6)}, ${coords.longitude.toFixed(6)} (±${Math.round(coords.accuracy)}m)`, ok: true });
      },
      (err) => {
        const e = GEO_ERRORS[err.code] ?? { title: '⚠ Location Error', detail: 'Unknown error. Refresh and try again.' };
        setLocMsg({ ...e, ok: false });
      },
      { enableHighAccuracy: true, timeout: 60000, maximumAge: 0 }
    );
  }, []);

  const handleDevBypassGps = () => {
    setUsedDevBypassGps(true);
    setLoc({ latitude: 0, longitude: 0, accuracy: 5 });
    setLocMsg({ title: '✓ Location Bypassed (DEV)', detail: 'Using mock coordinates', ok: true });
  };

  const initCamera = useCallback(async (signal?: AbortSignal, bypassFlag?: boolean) => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: 'user', width: { ideal: 640 }, height: { ideal: 480 } }, audio: false });
      if (signal?.aborted) {
        stream.getTracks().forEach(t => t.stop());
        return;
      }
      streamRef.current = stream;
      if (videoRef.current) { videoRef.current.srcObject = stream; videoRef.current.play().catch(() => {}); }
    } catch (err) {
      const isBypassed = bypassFlag !== undefined ? bypassFlag : devBypassEnabled;
      if (isBypassed) {
        flash('Camera error, but DEV bypass is available.', true);
        return;
      }
      if (signal?.aborted) return;
      const e = err as { name: string; message: string };
      const msgs: Record<string, string> = {
        NotAllowedError: 'Camera permission denied. Enable camera in browser settings and refresh.',
        NotFoundError: 'No camera found on this device.',
        NotReadableError: 'Camera is in use by another app. Close it and refresh.',
      };
      setErrMsg(msgs[e.name] ?? `Camera error: ${e.message}`);
      setStep('error');
    }
  }, [devBypassEnabled]);

  useEffect(() => {
    const ac = new AbortController();
    (async () => {
      try {
        const [storRes, sessRes] = await Promise.all([
          fetch(`${API}/api/storage-info`, { signal: ac.signal }),
          fetch(`${API}/s/${shortCode}/session`, { signal: ac.signal }),
        ]);
        const stor: StorageInfo = await storRes.json();
        const sessData = await sessRes.json();
        if (ac.signal.aborted) return;
        if (!sessRes.ok || !sessData.valid) throw new Error(sessData.message || 'Invalid or expired attendance link');
        if (new Date(sessData.session.expiresAt) < new Date()) throw new Error('This attendance session has expired');
        setStorageInfo(stor);
        setSession(sessData.session);
        setDevBypassEnabled(!!sessData.devBypassEnabled);
        setStep('form');
        await Promise.all([loadCaptcha(), initCamera(ac.signal, !!sessData.devBypassEnabled)]);
        if (!ac.signal.aborted) getLocation();
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
  }, [shortCode, API, loadCaptcha, initCamera, getLocation]);

  // Attach the (pre-acquired) camera stream once the form's <video> mounts.
  useEffect(() => {
    if (step === 'form' && streamRef.current && videoRef.current && !videoRef.current.srcObject) {
      videoRef.current.srcObject = streamRef.current;
      videoRef.current.play().catch(() => {});
    }
  }, [step]);

  const handleCapture = async () => {
    if (photoTaken) {
      photoDataRef.current = '';
      faceDetectedRef.current = true;
      setPhotoTaken(false);
      setUsedDevBypassCamera(false);
      if (videoRef.current) videoRef.current.style.display = 'block';
      if (canvasRef.current) canvasRef.current.style.display = 'none';
      return;
    }
    const video = videoRef.current; const canvas = canvasRef.current;
    if (!video || !canvas) return;
    const W = Math.min(video.videoWidth || 640, 800);
    const H = Math.round((video.videoHeight || 480) * (W / (video.videoWidth || 640)));
    canvas.width = W; canvas.height = H;
    canvas.getContext('2d')?.drawImage(video, 0, 0, W, H);
    photoDataRef.current = canvas.toDataURL('image/jpeg', 0.75);
    canvas.style.display = 'block';
    video.style.display = 'none';
    setPhotoTaken(true);

    // Face detection
    faceDetectedRef.current = true;
    if (typeof faceapi !== 'undefined') {
      try {
        if (!faceModelLoaded) await loadFaceModel();
        const detections = await faceapi.detectAllFaces(canvas, new faceapi.SsdMobilenetv1Options({ minConfidence: 0.5 }));
        faceDetectedRef.current = detections.length > 0;
        if (!faceDetectedRef.current) flash("Face not detected. Make sure your face is clearly visible before submitting.");
      } catch { faceDetectedRef.current = false; }
    } else {
      faceDetectedRef.current = false;
      flash("Face verification engine not ready. Attendance will be flagged.");
    }
  };

  const handleDevBypassCamera = () => {
    setUsedDevBypassCamera(true);
    photoDataRef.current = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
    faceDetectedRef.current = true;
    if (videoRef.current) videoRef.current.style.display = 'none';
    setPhotoTaken(true);
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!loc) { flash('Location required. Please allow location access.'); return; }
    const rollTrimmed = roll.trim().toUpperCase();
    if (!/^[A-Z0-9]{3,20}$/.test(rollTrimmed)) { flash('Invalid roll number format'); return; }
    setSubmitting(true);
    try {
      let body: Record<string, unknown>;
      if (storageInfo?.provider === 's3' && storageInfo.supportsDirectUpload) {
        const urlRes = await fetch(`${API}/s/${shortCode}/upload-url`);
        const urlData = await urlRes.json();
        if (!urlRes.ok) throw new Error(urlData.message || 'Failed to get upload URL');
        const blob = await (await fetch(photoDataRef.current)).blob();
        const upRes = await fetch(urlData.uploadUrl, { method: urlData.method, headers: urlData.headers || {}, body: blob });
        if (!upRes.ok) throw new Error('Photo upload failed');
        body = { studentName: name.trim(), rollNumber: roll.trim().toUpperCase(), directUpload: true, publicId: urlData.publicId, latitude: loc.latitude, longitude: loc.longitude, faceDetected: faceDetectedRef.current, captchaAnswer: captchaAnswer.trim(), captchaId, devBypassCamera: usedDevBypassCamera, devBypassGps: usedDevBypassGps };
      } else {
        body = { studentName: name.trim(), rollNumber: roll.trim().toUpperCase(), photo: photoDataRef.current, latitude: loc.latitude, longitude: loc.longitude, faceDetected: faceDetectedRef.current, captchaAnswer: captchaAnswer.trim(), captchaId, devBypassCamera: usedDevBypassCamera, devBypassGps: usedDevBypassGps };
      }

      const res = await fetch(`${API}/s/${shortCode}/submit`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) });
      const data = await res.json();
      if (!res.ok) throw new Error(data.message || 'Submission failed');

      streamRef.current?.getTracks().forEach(t => t.stop());
      const att = data.attendance;
      setDistanceInfo(`Distance: ${att.distanceFromLocation}m | ${att.verified ? 'VERIFIED - Within allowed area' : 'NOT VERIFIED - Outside allowed area'}`);
      setStep('success');
    } catch (err) {
      flash((err as Error).message);
      loadCaptcha();
      setSubmitting(false);
    }
  };

  const canSubmit = name.trim().length >= 2 && /^[A-Z0-9]{3,20}$/.test(roll.trim().toUpperCase()) && photoTaken && !!loc && captchaAnswer.trim().length >= 4;

  const Logo = (
    <div className="attend-pane-logo">
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#fff" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
      </svg>
    </div>
  );

  if (!isMobile && !devBypassEnabled && step !== 'loading' && step !== 'error') {
    return <MobileDeviceRequired />;
  }

  return (
    <>
    {step !== 'form' && (
    <div className="attend-page">
      {step === 'loading' && (
        <div className="attend-card"><div className="attend-center"><div className="spinner" /><p style={{ fontSize: 13, color: '#667085' }}>Loading session…</p></div></div>
      )}

      {step === 'error' && (
        <div className="attend-card">
          <div style={{ padding: 28 }}>
            <div className="attend-status err" style={{ marginBottom: 0, whiteSpace: 'pre-line' }}><span>{errMsg}</span></div>
            <p style={{ fontSize: 13, color: '#667085', marginTop: 12, textAlign: 'center' }}>Contact your instructor for assistance.</p>
          </div>
        </div>
      )}

      {step === 'success' && (
        <div className="attend-card">
          <div className="attend-center">
            <div style={{ fontSize: 56 }}>✅</div>
            <h3 style={{ fontSize: 19, fontWeight: 700, color: '#027a48' }}>Attendance Recorded!</h3>
            <p style={{ fontSize: 13, color: '#667085' }}>Your attendance has been successfully submitted.</p>
            {distanceInfo && <p style={{ fontSize: 12, color: '#667085', background: 'var(--color-bg-subtle)', borderRadius: 8, padding: 8, marginTop: 4 }}>{distanceInfo}</p>}
          </div>
        </div>
      )}
    </div>
    )}

      {step === 'form' && session && (
        <div className="attend-fullscreen">
          {/* Left: camera */}
          <div className="attend-camera-pane">
            <div className="attend-pane-inner">
              <div className="attend-pane-head">
                {Logo}
                <div>
                  <div className="attend-pane-title">Mark Attendance</div>
                  <div className="attend-pane-sub">{session.locationName}</div>
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
              <div className="attend-panel">
                <div className="attend-panel-label">Session expires</div>
                <div className="attend-panel-value">{new Date(session.expiresAt).toLocaleString()}</div>
              </div>
            </div>
          </div>

          {/* Right: form */}
          <div className="attend-form-pane">
            <div className="attend-pane-inner">
              {flashMsg && <div className={`attend-flash ${flashMsg.ok ? 'ok' : 'err'}`} style={{ boxShadow: 'none' }}>{flashMsg.text}</div>}
              <div>
                <h2>Confirm details</h2>
                <p className="sub">Fill in your details to mark attendance</p>
              </div>

              <div className={`attend-status ${locMsg.ok ? 'ok' : 'warn'}`}>
                <span>
                  <strong>{locMsg.title.replace(/[⚠✓]\s?/g, '')}</strong>
                  <div className="detail">{locMsg.detail}</div>
                  {!locMsg.ok && <button type="button" className="attend-btn-ghost" style={{ width: 'auto', padding: 0, marginTop: 4 }} onClick={getLocation}>Retry location</button>}
                  {devBypassEnabled && !locMsg.ok && (
                    <button type="button" className="attend-btn-ghost" onClick={handleDevBypassGps} style={{ width: 'auto', padding: '2px 8px', marginTop: 4, marginLeft: 8 }}>
                      🛠️ Mock Location
                    </button>
                  )}
                </span>
              </div>

              <form onSubmit={handleSubmit} style={{ display: 'flex', flexDirection: 'column', gap: 16 }}>
                <div className="attend-field">
                  <label>Full Name</label>
                  <input className="attend-input" placeholder="Enter your full name" value={name} onChange={(e) => setName(e.target.value)} required minLength={2} />
                </div>
                <div className="attend-field">
                  <label>Roll Number</label>
                  <input className="attend-input" placeholder="e.g. 21CS042" value={roll} onChange={(e) => setRoll(e.target.value.toUpperCase())} style={{ textTransform: 'uppercase' }} required pattern="[A-Z0-9]{3,20}" />
                </div>

                <div className="attend-field">
                  <label>Captcha</label>
                  <div className="attend-captcha">
                    <div ref={captchaRef} className="attend-captcha-svg" />
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
