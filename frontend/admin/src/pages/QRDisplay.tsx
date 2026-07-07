import { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import axios from 'axios';
import { Copy } from 'lucide-react';

// ponytail: no @types/qrcode installed — minimal typing for the CJS module
declare module 'qrcode' {
  export function toDataURL(text: string, options?: object): Promise<string>;
}

import * as QRCode from 'qrcode';

interface Session { _id: string; description?: string; expiresAt: string; isActive: boolean; totpEnabled?: boolean; }
interface TotpData { totpCode: string; shortLink: string; expiresAt: string; windowSeconds: number; }

const QRDisplay = () => {
  const { id: sessionId } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [session, setSession] = useState<Session | null>(null);
  const [totpData, setTotpData] = useState<TotpData | null>(null);
  const [loading, setLoading] = useState(true);
  const [qrDataUrl, setQrDataUrl] = useState('');
  const [countdown, setCountdown] = useState(0);
  const [error, setError] = useState('');
  const [paused, setPaused] = useState(false);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const generateQR = async (text: string) => {
    try { setQrDataUrl(await QRCode.toDataURL(text, { width: 400, margin: 2, color: { dark: '#000000', light: '#ffffff' } })); }
    catch { /* ignore */ }
  };

  const fetchTotp = useCallback(async () => {
    if (paused) return;
    try {
      const res = await axios.get<TotpData>(`/api/admin/sessions/${sessionId}/totp`);
      setTotpData(res.data);
      setLoading(false);
      const { protocol, hostname } = window.location;
      const fullUrl = `${protocol}//${hostname}/s/${res.data.shortLink}`;
      generateQR(fullUrl);
      const remaining = Math.max(0, Math.ceil((new Date(res.data.expiresAt).getTime() - Date.now()) / 1000));
      setCountdown(remaining);
    } catch { /* ignore */ }
  }, [sessionId, paused]);

  const fetchSession = useCallback(async () => {
    try {
      const res = await axios.get<Session>(`/api/admin/sessions/${sessionId}`);
      setSession(res.data);
      if (res.data.totpEnabled) { fetchTotp(); }
      else { setError('TOTP is not enabled for this session. Enable it by attaching a short link.'); setLoading(false); }
    } catch { setError('Failed to load session'); setLoading(false); }
  }, [sessionId, fetchTotp]);

  useEffect(() => { fetchSession(); }, [fetchSession]);

  useEffect(() => {
    if (paused) return;
    intervalRef.current = setInterval(fetchTotp, 1000);
    return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
  }, [fetchTotp, paused]);

  useEffect(() => {
    if (paused) return;
    const timer = setInterval(() => setCountdown((p) => Math.max(0, p - 1)), 1000);
    return () => clearInterval(timer);
  }, [paused]);

  if (loading) return (
    <div className="kiosk-centered">
      <div className="kiosk-spinner" />
      <p style={{ marginTop: '20px', color: '#666' }}>Loading QR code...</p>
    </div>
  );

  if (error) return (
    <div className="kiosk-centered">
      <div className="kiosk-error-card">
        <h2 style={{ marginBottom: '15px' }}>⚠️ {error}</h2>
        <p style={{ marginBottom: '20px' }}>Go to <strong>Short Links</strong> and attach one to this session.</p>
        <button className="btn btn-primary" onClick={() => navigate('/shortlinks')}>Go to Short Links</button>
      </div>
    </div>
  );

  const fullUrl = totpData ? `${window.location.protocol}//${window.location.hostname}/s/${totpData.shortLink}` : '';
  const progressPercent = ((totpData?.windowSeconds || 5) - countdown) / (totpData?.windowSeconds || 5) * 100;
  const isExpired = session && new Date(session.expiresAt) < new Date();
  const urgent = countdown <= 2;

  return (
    <div className={`kiosk-page${paused ? ' paused' : ''}`}>
      <div className="kiosk-header">
        <button className="btn btn-back" onClick={() => navigate(`/sessions/${sessionId}`)}>← Back to Session</button>
        <button className={`btn kiosk-pause-btn ${paused ? 'resume' : 'pause'}`} onClick={() => setPaused(!paused)}>
          {paused ? '▶ Resume' : '⏸ Pause'}
        </button>
      </div>

      {isExpired && <div className="kiosk-expired-banner"><strong>⚠️ Session has expired</strong></div>}

      <div className="kiosk-main">
        <div className="kiosk-code-panel">
          <div className="kiosk-code-label">Current Code</div>
          <div className="kiosk-code-value">{totpData?.totpCode || '------'}</div>
        </div>

        <div className={`kiosk-qr-panel${paused ? ' paused' : ''}`}>
          {qrDataUrl && <img src={qrDataUrl} alt="QR Code" />}
          <div className="kiosk-timer">
            <div className="kiosk-timer-row">
              <span>Next code in:</span>
              <span className={`value${urgent ? ' urgent' : ''}`}>{countdown}s</span>
            </div>
            <div className="kiosk-timer-bar">
              <div className={`kiosk-timer-fill${urgent ? ' urgent' : ''}`} style={{ width: `${progressPercent}%` }} />
            </div>
          </div>
        </div>

        <div className="kiosk-url-panel" onClick={() => navigator.clipboard.writeText(fullUrl)}>
          <span>{fullUrl}</span>
          <Copy size={18} />
        </div>

        <div className="kiosk-session-info">
          <p>Session: {session?.description || 'Attendance Session'}</p>
          <p>Expires: {session?.expiresAt ? new Date(session.expiresAt).toLocaleTimeString() : 'N/A'}</p>
        </div>
      </div>
    </div>
  );
};

export default QRDisplay;
