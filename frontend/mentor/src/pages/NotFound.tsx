import { Link } from 'react-router';

const NotFound = () => (
  <div className="login-screen">
    <div className="login-card" style={{ textAlign: 'center' }}>
      <h1 style={{ fontSize: 40, fontWeight: 800, margin: '0 0 8px' }}>404</h1>
      <p style={{ color: 'var(--color-muted)', marginBottom: 20 }}>Page not found.</p>
      <Link to="/" className="btn btn-primary btn-block" style={{ textDecoration: 'none' }}>Go Home</Link>
    </div>
  </div>
);

export default NotFound;
