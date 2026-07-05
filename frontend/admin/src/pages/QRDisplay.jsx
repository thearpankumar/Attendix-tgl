import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import axios from 'axios';
import QRCode from 'qrcode';
import { Copy } from 'lucide-react';

const QRDisplay = () => {
  const { id: sessionId } = useParams();
  const navigate = useNavigate();
  const [session, setSession] = useState(null);
  const [totpData, setTotpData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [qrDataUrl, setQrDataUrl] = useState('');
  const [countdown, setCountdown] = useState(0);
  const [error, setError] = useState('');
  const [paused, setPaused] = useState(false);
  const canvasRef = useRef(null);
  const intervalRef = useRef(null);

  const fetchSession = useCallback(async () => {
    try {
      const res = await axios.get(`/api/admin/sessions/${sessionId}`);
      setSession(res.data);

      if (res.data.totpEnabled) {
        fetchTotp();
      } else {
        setError('TOTP is not enabled for this session. Enable it by attaching a short link.');
        setLoading(false);
      }
    } catch (err) {
      setError('Failed to load session');
      setLoading(false);
    }
  }, [sessionId]);

  const fetchTotp = useCallback(async () => {
    if (paused) return;

    try {
      const res = await axios.get(`/api/admin/sessions/${sessionId}/totp`);
      setTotpData(res.data);
      setLoading(false);

      const fullUrl = `${window.location.origin}/s/${res.data.shortLink}`;
      generateQR(fullUrl);

      const now = Date.now();
      const expires = new Date(res.data.expiresAt).getTime();
      const remaining = Math.max(0, Math.ceil((expires - now) / 1000));
      setCountdown(remaining);
    } catch (err) {
      console.error('TOTP fetch error:', err);
    }
  }, [sessionId, paused]);

  const generateQR = async (text) => {
    try {
      const dataUrl = await QRCode.toDataURL(text, {
        width: 400,
        margin: 2,
        color: {
          dark: '#000000',
          light: '#ffffff',
        },
      });
      setQrDataUrl(dataUrl);
    } catch (err) {
      console.error('QR generation error:', err);
    }
  };

  useEffect(() => {
    fetchSession();
  }, [fetchSession]);

  useEffect(() => {
    if (paused) return;

    intervalRef.current = setInterval(() => {
      fetchTotp();
    }, 1000);

    return () => {
      if (intervalRef.current) {
        clearInterval(intervalRef.current);
      }
    };
  }, [fetchTotp, paused]);

  useEffect(() => {
    if (paused) return;

    const timer = setInterval(() => {
      setCountdown((prev) => Math.max(0, prev - 1));
    }, 1000);

    return () => clearInterval(timer);
  }, [paused]);

  const formatTime = (date) => new Date(date).toLocaleTimeString();

  if (loading) {
    return (
      <div className="kiosk-centered">
        <div className="kiosk-spinner"></div>
        <p style={{ marginTop: '20px', color: '#666' }}>Loading QR code...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="kiosk-centered">
        <div className="kiosk-error-card">
          <h2 style={{ marginBottom: '15px' }}>⚠️ {error}</h2>
          <p style={{ marginBottom: '20px' }}>
            Go to <strong>Short Links</strong> and attach one to this session.
          </p>
          <button className="btn btn-primary" onClick={() => navigate('/shortlinks')}>
            Go to Short Links
          </button>
        </div>
      </div>
    );
  }

  const fullUrl = totpData ? `${window.location.origin}/s/${totpData.shortLink}` : '';
  const progressPercent = ((totpData?.windowSeconds || 5) - countdown) / (totpData?.windowSeconds || 5) * 100;
  const isExpired = session && new Date(session.expiresAt) < new Date();
  const urgent = countdown <= 2;

  return (
    <div className={`kiosk-page${paused ? ' paused' : ''}`}>
      <div className="kiosk-header">
        <button className="btn btn-back" onClick={() => navigate(`/sessions/${sessionId}`)}>
          ← Back to Session
        </button>

        <button className={`btn kiosk-pause-btn ${paused ? 'resume' : 'pause'}`} onClick={() => setPaused(!paused)}>
          {paused ? '▶ Resume' : '⏸ Pause'}
        </button>
      </div>

      {isExpired && (
        <div className="kiosk-expired-banner">
          <strong>⚠️ Session has expired</strong>
        </div>
      )}

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
              <div className={`kiosk-timer-fill${urgent ? ' urgent' : ''}`} style={{ width: `${progressPercent}%` }}></div>
            </div>
          </div>
        </div>

        <div className="kiosk-url-panel" onClick={() => navigator.clipboard.writeText(fullUrl)}>
          <span>{fullUrl}</span>
          <Copy size={18} />
        </div>

        <div className="kiosk-session-info">
          <p>Session: {session?.description || 'Attendance Session'}</p>
          <p>Expires: {session?.expiresAt ? formatTime(session.expiresAt) : 'N/A'}</p>
        </div>
      </div>
    </div>
  );
};

export default QRDisplay;
