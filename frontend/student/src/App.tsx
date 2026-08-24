import { BrowserRouter, Routes, Route } from 'react-router';
import StudentScan from './pages/StudentScan';
import LegacyAttend from './pages/LegacyAttend';
import ExtensionPair from './pages/ExtensionPair';
function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="/attend/:shortCode" element={<StudentScan />} />
        <Route path="/attend/legacy/:shortCode" element={<LegacyAttend />} />
        <Route path="/attend/pair/:shortCode/:pairingCode" element={<ExtensionPair />} />
        <Route path="/s/:shortCode" element={<StudentScan />} />
        <Route path="*" element={<div style={{ padding: 24 }}>Not found</div>} />
      </Routes>
    </BrowserRouter>
  );
}

export default App;
