import { NavLink } from 'react-router-dom';
import { LogOut } from 'lucide-react';
import { useAuth } from '../../context/AuthContext';
import { navLinks } from './navLinks';

const Sidebar = () => {
  const { admin, logout } = useAuth();

  return (
    <aside className="sidebar">
      <div className="sidebar-header">Attendance System</div>
      <nav className="sidebar-nav">
        {navLinks.map(({ to, label, icon: Icon, end, danger }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              `sidebar-link${isActive ? ' active' : ''}${danger ? ' danger' : ''}`
            }
          >
            <Icon size={18} />
            {label}
          </NavLink>
        ))}
      </nav>
      <div className="sidebar-footer">
        <span className="sidebar-username">{admin?.username}</span>
        <button className="btn btn-secondary btn-small" onClick={logout}>
          <LogOut size={14} />
        </button>
      </div>
    </aside>
  );
};

export default Sidebar;
