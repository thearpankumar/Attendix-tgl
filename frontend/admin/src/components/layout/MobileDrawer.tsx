import { NavLink } from 'react-router';
import { X, LogOut, Shield, Sun, Moon } from 'lucide-react';
import { useAuth } from '../../context/AuthContext';
import { useTheme } from '../../hooks/useTheme';
import { useFlaggedCount } from '../../hooks/useFlaggedCount';
import { navLinks } from './navLinks';

const MobileDrawer = ({ onClose }: { onClose: () => void }) => {
  const { admin, logout } = useAuth();
  const { theme, toggleTheme } = useTheme();
  const flaggedCount = useFlaggedCount();

  return (
    <>
      <div className="drawer-overlay" onClick={onClose} />
      <div className="drawer">
        <div className="drawer-header">
          <div className="sidebar-logo">
            <Shield size={18} />
            <span>Attendix</span>
          </div>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
            <button className="theme-toggle-btn" onClick={toggleTheme} aria-label="Toggle theme">
              {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
            </button>
            <button className="modal-close" onClick={onClose} aria-label="Close menu">
              <X size={22} />
            </button>
          </div>
        </div>
        <nav className="drawer-nav">
          {navLinks.map(({ to, label, icon: Icon, end, danger }) => (
            <NavLink
              key={to}
              to={to}
              end={end}
              onClick={onClose}
              className={({ isActive }) =>
                `sidebar-link${isActive ? ' active' : ''}${danger ? ' danger' : ''}`
              }
            >
              <Icon size={18} />
              {label}
              {to === '/flagged' && flaggedCount > 0 && (
                <span className="sidebar-badge">{flaggedCount > 99 ? '99+' : flaggedCount}</span>
              )}
            </NavLink>
          ))}
          <button className="sidebar-link danger" onClick={logout}>
            <LogOut size={18} />
            Logout
          </button>
        </nav>
        <div className="sidebar-footer">
          <span className="sidebar-username">{admin?.username}</span>
        </div>
      </div>
    </>
  );
};

export default MobileDrawer;
