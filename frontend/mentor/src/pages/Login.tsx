import { useState } from 'react';
import type { FormEvent } from 'react';
import { Navigate } from 'react-router';
import { Loader2 } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

const CampusIllustration = () => (
  <svg viewBox="0 0 240 150" role="presentation" aria-hidden="true" className="login-illustration-svg">
    <circle cx="120" cy="95" r="70" fill="var(--color-primary-light)" />
    <circle cx="36" cy="112" r="16" fill="var(--color-primary-light)" />
    <circle cx="206" cy="108" r="12" fill="var(--color-primary-light)" />
    <rect x="188" y="70" width="3" height="42" rx="1.5" fill="var(--color-border)" />
    <circle cx="189.5" cy="66" r="7" fill="var(--color-primary-light)" stroke="var(--color-primary)" strokeWidth="1.5" />
    <rect x="30" y="112" width="180" height="4" rx="2" fill="var(--color-border)" />
    <polygon points="120,28 168,66 72,66" fill="var(--color-primary)" opacity="0.85" />
    <rect x="66" y="66" width="108" height="10" fill="var(--color-primary-dark)" opacity="0.9" />
    <rect x="60" y="76" width="120" height="36" fill="var(--color-surface)" stroke="var(--color-primary)" strokeWidth="1.5" />
    {[74, 96, 118, 140, 162].map((x) => (
      <rect key={x} x={x} y={80} width="8" height="28" fill="var(--color-primary)" opacity="0.75" />
    ))}
    <rect x="54" y="112" width="132" height="8" fill="var(--color-primary-dark)" opacity="0.9" />
  </svg>
);

const Login = () => {
  const { admin, login } = useAuth();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [submitting, setSubmitting] = useState(false);

  if (admin) return <Navigate to="/" />;

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    setSubmitting(true);
    const result = await login(username, password);
    setSubmitting(false);
    if (!result.success) setError(result.message || 'Login failed');
  };

  return (
    <div className="login-screen">
      <div className="login-card">
        <div className="login-illustration">
          <CampusIllustration />
          <h1>Exam Attendance Portal</h1>
          <p>Please login to continue</p>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="field">
            <label htmlFor="username">Username</label>
            <input
              id="username" type="text" value={username} autoFocus autoComplete="username"
              onChange={(e) => setUsername(e.target.value)} required
            />
          </div>
          <div className="field">
            <label htmlFor="password">Password</label>
            <input
              id="password" type="password" value={password} autoComplete="current-password"
              onChange={(e) => setPassword(e.target.value)} required
            />
          </div>

          {error && (
            <div style={{
              background: 'var(--color-danger-bg)', color: 'var(--color-danger-txt)',
              borderRadius: 10, padding: '10px 14px', fontSize: 13, marginBottom: 16,
            }}>{error}</div>
          )}

          <button type="submit" className="btn btn-primary btn-block" disabled={submitting}>
            {submitting ? <Loader2 size={16} className="spin" /> : null}
            {submitting ? 'Signing in…' : 'Sign In'}
          </button>
        </form>
      </div>
    </div>
  );
};

export default Login;
