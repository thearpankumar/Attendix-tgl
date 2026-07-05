import { NavLink } from 'react-router-dom';
import { X, LogOut } from 'lucide-react';
import { useAuth } from '../../context/AuthContext';
import { navLinks } from './navLinks';

const MobileDrawer = ({ onClose }) => {
  const { admin, logout } = useAuth();

  return (
    <>
      <div className="drawer-overlay" onClick={onClose} />
      <div className="drawer">
        <div className="drawer-header">
          <span>{admin?.username}</span>
          <button className="modal-close" onClick={onClose} aria-label="Close menu">
            <X size={22} />
          </button>
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
            </NavLink>
          ))}
          <button className="sidebar-link danger" onClick={logout}>
            <LogOut size={18} />
            Logout
          </button>
        </nav>
      </div>
    </>
  );
};

export default MobileDrawer;
