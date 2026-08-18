import { Link, useLocation } from 'react-router';
import { ClipboardCheck, Settings as SettingsIcon, LogOut, ArrowLeft } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

const Topbar = () => {
  const { admin, logout } = useAuth();
  const location = useLocation();
  const isSubPage = location.pathname !== '/';

  const displayName = admin?.fullName || admin?.username || '';

  return (
    <header className="topbar">
      {isSubPage ? (
        <Link to="/" className="topbar-brand" style={{ textDecoration: 'none' }}>
          <ArrowLeft size={18} />
          <span>Back</span>
        </Link>
      ) : (
        <div className="topbar-brand">
          {/* Sized to span both lines, so it sits between "Attendix" and
              the username below it rather than only lining up with the
              top line — a long name used to crowd "Attendix" on one line. */}
          <ClipboardCheck size={34} color="var(--color-primary)" />
          <div className="topbar-brand-col">
            <span>Attendix</span>
            {displayName && <span className="topbar-username">{displayName}</span>}
          </div>
        </div>
      )}
      <div className="topbar-actions">
        {isSubPage && displayName && (
          <span style={{ fontSize: 13, color: 'var(--color-muted)', marginRight: 4 }}>{displayName}</span>
        )}
        <Link to="/settings" className="icon-btn" aria-label="Settings"><SettingsIcon size={16} /></Link>
        <button className="icon-btn" onClick={logout} aria-label="Log out"><LogOut size={16} /></button>
      </div>
    </header>
  );
};

export default Topbar;
