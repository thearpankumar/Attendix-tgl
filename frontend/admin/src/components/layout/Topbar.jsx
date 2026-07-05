import { Menu } from 'lucide-react';

const Topbar = ({ onMenuClick }) => (
  <header className="topbar">
    <span className="topbar-title">Attendance System</span>
    <button className="hamburger-btn" onClick={onMenuClick} aria-label="Open menu">
      <Menu size={24} />
    </button>
  </header>
);

export default Topbar;
