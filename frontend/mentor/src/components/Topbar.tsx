import { Link, useLocation } from 'react-router';
import { ClipboardCheck, Settings as SettingsIcon, LogOut, ArrowLeft } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

const Topbar = () => {
  const { admin, logout } = useAuth();
  const location = useLocation();
  const isSubPage = location.pathname !== '/';

  return (
    <header className="topbar">
      {isSubPage ? (
        <Link to="/" className="topbar-brand" style={{ textDecoration: 'none' }}>
          <ArrowLeft size={18} />
          <span>Back</span>
        </Link>
      ) : (
        <div className="topbar-brand">
          <ClipboardCheck size={20} color="var(--color-primary)" />
          <span>Attendix</span>
        </div>
      )}
      <div className="topbar-actions">
        {admin && (
          <span style={{ fontSize: 13, color: 'var(--color-muted)', marginRight: 4 }}>
            {admin.fullName || admin.username}
          </span>
        )}
        <Link to="/settings" className="icon-btn" aria-label="Settings"><SettingsIcon size={16} /></Link>
        <button className="icon-btn" onClick={logout} aria-label="Log out"><LogOut size={16} /></button>
      </div>
    </header>
  );
};

export default Topbar;
