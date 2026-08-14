import { ShieldAlert } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

const Forbidden = () => {
  const { admin, logout } = useAuth();

  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', minHeight: '70vh' }}>
      <div className="card" style={{ maxWidth: 420, textAlign: 'center', padding: '2.5rem 2rem' }}>
        <div style={{
          width: 56, height: 56, borderRadius: 14, display: 'flex', alignItems: 'center', justifyContent: 'center',
          background: 'rgba(239,68,68,0.12)', color: '#ef4444', margin: '0 auto 1.25rem',
        }}>
          <ShieldAlert size={28} />
        </div>
        <h3 style={{ marginBottom: '0.5rem' }}>This panel is for super admins</h3>
        <p style={{ color: 'var(--color-muted)', lineHeight: 1.6, marginBottom: '1.5rem' }}>
          {admin?.username ? <>Signed in as <strong>{admin.username}</strong> (mentor). </> : null}
          Mentors mark attendance from the dedicated mentor app at <strong>/mark-attendance</strong>, not this panel.
        </p>
        <button className="btn btn-secondary" onClick={logout}>Log out</button>
      </div>
    </div>
  );
};

export default Forbidden;
