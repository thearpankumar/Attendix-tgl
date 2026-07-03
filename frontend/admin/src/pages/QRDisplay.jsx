import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import axios from 'axios';
import QRCode from 'qrcode';

const QRDisplay = () => {
  const { sessionId } = useParams();
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
      setCountdown((prev) => {
        const newVal = Math.max(0, prev - 1);
        return newVal;
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

  const fullUrl = totpData ? `${window.location.origin}/s/${totpData.shortLink}` : '';
  const progressPercent = ((totpData?.windowSeconds || 5) - countdown) / (totpData?.windowSeconds || 5) * 100;
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
        }}>
          <div style={{ color: 'rgba(255,255,255,0.7)', fontSize: '14px', marginBottom: '5px' }}>
            Current Code
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
        </div>

        {/* QR Code */}
        <div style={{
          background: 'white',
          padding: '30px',
          borderRadius: '16px',
          boxShadow: '0 10px 40px rgba(0,0,0,0.3)',
          marginBottom: '20px',
          opacity: paused ? 0.5 : 1,
        }}>
          {qrDataUrl && (
            <img 
              src={qrDataUrl} 
              alt="QR Code" 
              style={{ width: '350px', height: '350px' }}
            />
          )}
          
          {/* Timer Bar */}
          <div style={{ marginTop: '20px' }}>
            <div style={{
              display: 'flex',
              justifyContent: 'space-between',
              marginBottom: '8px',
              fontSize: '14px',
              color: '#666',
            }}>
              <span>Next code in:</span>
              <span style={{ fontWeight: 'bold', color: countdown <= 2 ? '#e74c3c' : '#667eea' }}>
                {countdown}s
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
                width: `${progressPercent}%`,
                background: countdown <= 2 ? '#e74c3c' : '#667eea',
                transition: 'width 0.5s linear',
              }}></div>
            </div>
          </div>
        </div>

        {/* URL Display */}
        <div style={{
          background: 'white',
          padding: '15px 25px',
          borderRadius: '8px',
          marginBottom: '20px',
          display: 'flex',
          alignItems: 'center',
          gap: '10px',
          cursor: 'pointer',
        }}
        onClick={() => navigator.clipboard.writeText(fullUrl)}
        >
          <span style={{ fontSize: '18px', color: '#333' }}>{fullUrl}</span>
          <span style={{ fontSize: '20px' }}>📋</span>
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
