import { useState } from 'react';
import Sidebar from './Sidebar';
import Topbar from './Topbar';
import MobileDrawer from './MobileDrawer';

const AppShell = ({ children }) => {
  const [drawerOpen, setDrawerOpen] = useState(false);

  return (
    <div className="app-shell">
      <Sidebar />
      <Topbar onMenuClick={() => setDrawerOpen(true)} />
      {drawerOpen && <MobileDrawer onClose={() => setDrawerOpen(false)} />}
      <main className="app-main">{children}</main>
    </div>
  );
};

export default AppShell;
