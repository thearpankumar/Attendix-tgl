import { BrowserRouter, Routes, Route } from 'react-router-dom';
import StudentScan from './pages/StudentScan';
import LegacyAttend from './pages/LegacyAttend';

function App() {
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
