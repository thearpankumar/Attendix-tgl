import { NavLink } from 'react-router';
import { LogOut, Shield, Sun, Moon, ChevronLeft } from 'lucide-react';
import { useAuth } from '../../context/AuthContext';
import { useTheme } from '../../hooks/useTheme';
import { useFlaggedCount } from '../../hooks/useFlaggedCount';
import { navLinks } from './navLinks';

interface SidebarProps {
  collapsed: boolean;
  onToggle: () => void;
}

const Sidebar = ({ collapsed, onToggle }: SidebarProps) => {
  const { admin, logout } = useAuth();
  const { theme, toggleTheme } = useTheme();
  const { count: flaggedCount, hasHighSeverityUnreviewed } = useFlaggedCount();

  return (
    <aside className="sidebar" data-collapsed={collapsed ? 'true' : 'false'}>
      {/* ── Header ───────────────────────────────────── */}
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <Shield size={20} />
          <span className="sidebar-label">Attendix</span>
        </div>

        <button
          className="theme-toggle-btn"
          onClick={toggleTheme}
          aria-label="Toggle theme"
          title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
        >
          {theme === 'dark' ? <Sun size={16} /> : <Moon size={16} />}
        </button>
      </div>

      {/* ── Nav ──────────────────────────────────────── */}
      <nav className="sidebar-nav">
        {navLinks.map(({ to, label, icon: Icon, end, danger }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              `sidebar-link${isActive ? ' active' : ''}${danger ? ' danger' : ''}`
            }
            title={collapsed ? label : undefined}
          >
            <Icon size={18} />
            <span className="sidebar-label">{label}</span>
            {to === '/flagged' && flaggedCount > 0 && (
              <span
                className="sidebar-badge"
                style={hasHighSeverityUnreviewed ? { background: 'var(--color-danger)', animation: 'pulse 1.8s ease-in-out infinite' } : undefined}
                title={hasHighSeverityUnreviewed ? 'Includes high-severity unreviewed flags' : undefined}
              >
                {flaggedCount > 99 ? '99+' : flaggedCount}
              </span>
            )}
          </NavLink>
        ))}
      </nav>

      {/* ── Footer ───────────────────────────────────── */}
      <div className="sidebar-footer">
        <span className="sidebar-label" style={{ display: 'flex', flexDirection: 'column', gap: 2, minWidth: 0 }}>
          <span className="sidebar-username" style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{admin?.username}</span>
          {admin?.role && (
            <span style={{ fontSize: 10, fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.04em', color: 'var(--color-primary)' }}>
              {admin.role === 'super_admin' ? 'Super Admin' : admin.role}
            </span>
          )}
        </span>
        <button
          className="btn btn-secondary btn-small"
          onClick={logout}
          title="Logout"
          aria-label="Logout"
        >
          <LogOut size={14} />
        </button>
      </div>

      {/* ── Collapse Edge Button ─────────────────────── */}
      <button
        className="sidebar-collapse-btn"
        onClick={onToggle}
        aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
      >
        <ChevronLeft size={14} />
      </button>
    </aside>
  );
};

export default Sidebar;
