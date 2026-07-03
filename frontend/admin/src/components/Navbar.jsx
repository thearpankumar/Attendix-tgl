import { Link } from 'react-router-dom';
import { useAuth } from '../context/AuthContext';

const Navbar = () => {
  const { admin, logout } = useAuth();

  return (
    <nav className="navbar">
      <h1>Attendance System</h1>
      <div className="navbar-nav">
        <Link to="/" className="nav-link">
          Dashboard
        </Link>
        <Link to="/locations" className="nav-link">
          Locations
        </Link>
        <Link to="/sessions" className="nav-link">
          Sessions
        </Link>
        <Link to="/shortlinks" className="nav-link">
          Short Links
        </Link>
        <Link to="/webauthn" className="nav-link">
          Biometrics
        </Link>
        <Link to="/flagged" className="nav-link" style={{ color: '#e74c3c' }}>
          ⚠ Flags
        </Link>
        <span style={{ color: '#666', marginLeft: '10px' }}>
          {admin?.username}
        </span>
        <button className="btn btn-secondary btn-small" onClick={logout}>
          Logout
        </button>
      </div>
    </nav>
  );
};

export default Navbar;
