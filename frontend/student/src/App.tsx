import { BrowserRouter, Routes, Route } from 'react-router-dom';
import StudentScan from './pages/StudentScan';
import LegacyAttend from './pages/LegacyAttend';
import { useIsMobile } from './hooks/useIsMobile';
import MobileDeviceRequired from './components/MobileDeviceRequired';

function App() {
  const isMobile = useIsMobile();
  const bypassMobileCheck = import.meta.env.VITE_BYPASS_MOBILE_CHECK === 'true';

  if (!isMobile && !bypassMobileCheck) {
    return <MobileDeviceRequired />;
  }

  return (
    <BrowserRouter>
      <Routes>
        <Route path="/attend/:shortCode" element={<StudentScan />} />
        <Route path="/attend/legacy/:shortCode" element={<LegacyAttend />} />
        <Route path="/s/:shortCode" element={<StudentScan />} />
        <Route path="*" element={<div style={{ padding: 24 }}>Not found</div>} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
