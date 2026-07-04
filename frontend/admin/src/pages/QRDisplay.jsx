import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import axios from 'axios';
import QRCode from 'qrcode';

const QR_WINDOW_SECONDS = 4;

const QRDisplay = () => {
  const { sessionId } = useParams();
  const navigate = useNavigate();
  const [session, setSession] = useState(null);
  const [totpData, setTotpData] = useState(null);
  const [loading, setLoading] = useState(true);
  const [qrDataUrl, setQrDataUrl] = useState('');
  const [totpCountdown, setTotpCountdown] = useState(0);
  const [qrCountdown, setQrCountdown] = useState(QR_WINDOW_SECONDS);
  const [error, setError] = useState('');
  const [paused, setPaused] = useState(false);
  // Track last qrToken slot so we only regenerate QR when token changes
  const lastQrTokenRef = useRef('');
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
      
      // Update TOTP countdown from server
      const now = Date.now();
      const expires = new Date(res.data.expiresAt).getTime();
      const remaining = Math.max(0, Math.ceil((expires - now) / 1000));
      setTotpCountdown(remaining);

      // Regenerate QR only when qrToken changes (every 4 seconds)
      const qrToken = res.data.qrToken;
      if (qrToken && qrToken !== lastQrTokenRef.current && res.data.shortLink) {
        lastQrTokenRef.current = qrToken;
        const qrUrl = `${window.location.origin}/s/${res.data.shortLink}?qrt=${encodeURIComponent(qrToken)}`;
        generateQR(qrUrl);
        // Reset QR countdown to full 4 seconds on each new token
        setQrCountdown(QR_WINDOW_SECONDS);
      } else if (!qrToken && res.data.shortLink) {
        // Fallback: no QRT support — use plain URL
        const fallbackUrl = `${window.location.origin}/s/${res.data.shortLink}`;
        if (fallbackUrl !== lastQrTokenRef.current) {
          lastQrTokenRef.current = fallbackUrl;
          generateQR(fallbackUrl);
        }
      }
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

  // Poll TOTP every second (picks up new qrToken automatically every 4 seconds)
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

  // Tick TOTP countdown locally between polls
  useEffect(() => {
    if (paused) return;
    
    const timer = setInterval(() => {
      setTotpCountdown((prev) => Math.max(0, prev - 1));
    }, 1000);
    
    return () => clearInterval(timer);
  }, [paused]);

  // Tick QR countdown locally — purely cosmetic, real rotation is token-driven
  useEffect(() => {
    if (paused) return;
    
    const timer = setInterval(() => {
      setQrCountdown((prev) => {
        if (prev <= 1) return QR_WINDOW_SECONDS; // will be overridden by fetchTotp reset anyway
        return prev - 1;
      });
    }, 1000);
    
    return () => clearInterval(timer);
  }, [paused]);

  const formatTime = (date) => {
    return new Date(date).toLocaleTimeString();
  };

  if (loading) {
    return (
      <div style={{ 
        display: 'flex', 
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        flexDirection: 'column'
      }}>
        <div className="spinner" style={{
          width: '50px',
          height: '50px',
          border: '4px solid #f3f3f3',
          borderTop: '4px solid #667eea',
          borderRadius: '50%',
          animation: 'spin 1s linear infinite',
        }}></div>
        <p style={{ marginTop: '20px', color: '#666' }}>Loading QR code...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div style={{ 
        display: 'flex', 
        justifyContent: 'center', 
        alignItems: 'center', 
        height: '100vh',
        flexDirection: 'column'
      }}>
        <div style={{ 
          background: '#f8d7da', 
          color: '#721c24', 
          padding: '30px',
          borderRadius: '12px',
          textAlign: 'center',
          maxWidth: '500px'
        }}>
          <h2 style={{ marginBottom: '15px' }}>⚠️ {error}</h2>
          <p style={{ marginBottom: '20px' }}>
            Go to <strong>Short Links</strong> and attach one to this session.
          </p>
          <button 
            className="btn btn-primary"
            onClick={() => navigate('/shortlinks')}
          >
            Go to Short Links
          </button>
        </div>
      </div>
    );
  }

  const baseUrl = totpData?.shortLink
    ? `${window.location.origin}/s/${totpData.shortLink}`
    : '';
  const totpProgressPercent = ((totpData?.windowSeconds || 5) - totpCountdown) / (totpData?.windowSeconds || 5) * 100;
  const qrProgressPercent = ((QR_WINDOW_SECONDS - qrCountdown) / QR_WINDOW_SECONDS) * 100;
  const isExpired = session && new Date(session.expiresAt) < new Date();

  return (
    <div style={{ 
      minHeight: '100vh',
      background: paused ? '#f0f0f0' : 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
      padding: '20px',
    }}>
      {/* Header */}
      <div style={{
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        marginBottom: '20px',
      }}>
        <button 
          className="btn btn-secondary"
          onClick={() => navigate(`/sessions/${sessionId}`)}
          style={{ background: 'white', color: '#333' }}
        >
          ← Back to Session
        </button>
        
        <button 
          className="btn"
          onClick={() => setPaused(!paused)}
          style={{ 
            background: paused ? '#27ae60' : '#e74c3c',
            color: 'white',
          }}
        >
          {paused ? '▶ Resume' : '⏸ Pause'}
        </button>
      </div>

      {isExpired && (
        <div style={{
          background: '#f8d7da',
          color: '#721c24',
          padding: '15px',
          borderRadius: '8px',
          textAlign: 'center',
          marginBottom: '20px',
        }}>
          <strong>⚠️ Session has expired</strong>
        </div>
      )}

      {/* Main QR Container */}
      <div style={{
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        maxWidth: '600px',
        margin: '0 auto',
      }}>
        {/* TOTP Code Display */}
        <div style={{
          background: 'rgba(255,255,255,0.1)',
          padding: '15px 30px',
          borderRadius: '8px',
          marginBottom: '20px',
          textAlign: 'center',
          width: '100%',
        }}>
          <div style={{ color: 'rgba(255,255,255,0.7)', fontSize: '14px', marginBottom: '5px' }}>
            Students enter this code on the form
          </div>
          <div style={{
            fontSize: '48px',
            fontWeight: '600',
            letterSpacing: '8px',
            color: 'white',
            fontFamily: 'Courier New, monospace',
          }}>
            {totpData?.totpCode || '------'}
          </div>
          {/* TOTP timer bar */}
          <div style={{ marginTop: '10px' }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              fontSize: '12px',
              color: 'rgba(255,255,255,0.6)',
              marginBottom: '4px',
            }}>
              <span>Code changes in</span>
              <span style={{ fontWeight: 'bold', color: totpCountdown <= 2 ? '#ff6b6b' : 'rgba(255,255,255,0.9)' }}>
                {totpCountdown}s
              </span>
            </div>
            <div style={{ height: '4px', background: 'rgba(255,255,255,0.2)', borderRadius: '2px', overflow: 'hidden' }}>
              <div style={{
                height: '100%',
                width: `${100 - totpProgressPercent}%`,
                background: totpCountdown <= 2 ? '#ff6b6b' : 'rgba(255,255,255,0.8)',
                transition: 'width 0.5s linear',
              }}></div>
            </div>
          </div>
        </div>

        {/* QR Code */}
        <div style={{
          background: 'white',
          padding: '30px',
          borderRadius: '16px',
          boxShadow: '0 10px 40px rgba(0,0,0,0.3)',
          marginBottom: '12px',
          opacity: paused ? 0.5 : 1,
          position: 'relative',
        }}>
          {qrDataUrl && (
            <img 
              src={qrDataUrl} 
              alt="QR Code" 
              style={{ width: '350px', height: '350px', display: 'block' }}
            />
          )}
          
          {/* QR rotation timer bar */}
          <div style={{ marginTop: '16px' }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              marginBottom: '6px',
              fontSize: '13px',
              color: '#555',
            }}>
              <span style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
                🔄 <strong>QR expires in</strong>
              </span>
              <span style={{
                fontWeight: 'bold',
                color: qrCountdown <= 1 ? '#e74c3c' : '#667eea',
                fontSize: '15px',
              }}>
                {qrCountdown}s
              </span>
            </div>
            <div style={{
              height: '8px',
              background: '#eee',
              borderRadius: '4px',
              overflow: 'hidden',
            }}>
              <div style={{
                height: '100%',
                width: `${100 - qrProgressPercent}%`,
                background: qrCountdown <= 1 ? '#e74c3c' : '#667eea',
                transition: 'width 0.5s linear',
              }}></div>
            </div>
            <div style={{ fontSize: '11px', color: '#999', marginTop: '5px', textAlign: 'center' }}>
              Anti-sharing: QR rotates every {QR_WINDOW_SECONDS}s — screenshots won't work
            </div>
          </div>
        </div>

        {/* Base URL display (without token — just for reference) */}
        <div style={{
          background: 'rgba(255,255,255,0.15)',
          padding: '10px 20px',
          borderRadius: '8px',
          marginBottom: '20px',
          display: 'flex',
          alignItems: 'center',
          gap: '10px',
          cursor: 'pointer',
          width: '100%',
        }}
        onClick={() => navigator.clipboard.writeText(baseUrl)}
        title="Copy base URL (without QR token)"
        >
          <span style={{ fontSize: '14px', color: 'rgba(255,255,255,0.9)', flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
            {baseUrl}
          </span>
          <span style={{ fontSize: '16px' }}>📋</span>
        </div>

        {/* Session Info */}
        <div style={{
          color: 'rgba(255,255,255,0.8)',
          textAlign: 'center',
          fontSize: '14px',
        }}>
          <p>Session: {session?.description || 'Attendance Session'}</p>
          <p>Expires: {session?.expiresAt ? formatTime(session.expiresAt) : 'N/A'}</p>
        </div>
      </div>

      <style>{`
        @keyframes spin {
          0% { transform: rotate(0deg); }
          100% { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
};

export default QRDisplay;
