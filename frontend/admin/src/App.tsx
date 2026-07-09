import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ToastContainer } from 'react-toastify';
import { useAuth } from './context/AuthContext';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import Locations from './pages/Locations';
import Sessions from './pages/Sessions';
import SessionDetail from './pages/SessionDetail';
import ShortLinks from './pages/ShortLinks';
import QRDisplay from './pages/QRDisplay';
import FlaggedAttendance from './pages/FlaggedAttendance';
import WebAuthnCredentials from './pages/WebAuthnCredentials';
import NotFound from './pages/NotFound';
import AppShell from './components/layout/AppShell';
import type { ReactNode } from 'react';

const PrivateRoute = ({ children }: { children: ReactNode }) => {
  const { admin, loading } = useAuth();
  if (loading) return <div className="loading">Loading...</div>;
  if (!admin) return <Navigate to="/login" />;
  return <AppShell>{children}</AppShell>;
};

function App() {
  const { admin } = useAuth();

  return (
    <BrowserRouter basename="/admin">
      <Routes>
        <Route path="/login" element={admin ? <Navigate to="/" /> : <Login />} />
        <Route path="/" element={<PrivateRoute><Dashboard /></PrivateRoute>} />
        <Route path="/locations" element={<PrivateRoute><Locations /></PrivateRoute>} />
        <Route path="/sessions" element={<PrivateRoute><Sessions /></PrivateRoute>} />
        <Route path="/sessions/:id" element={<PrivateRoute><SessionDetail /></PrivateRoute>} />
        <Route path="/sessions/:id/qr" element={admin ? <QRDisplay /> : <Navigate to="/login" />} />
        <Route path="/shortlinks" element={<PrivateRoute><ShortLinks /></PrivateRoute>} />
        <Route path="/flagged" element={<PrivateRoute><FlaggedAttendance /></PrivateRoute>} />
        <Route path="/webauthn" element={<PrivateRoute><WebAuthnCredentials /></PrivateRoute>} />
        <Route path="*" element={<NotFound />} />
      </Routes>
      <ToastContainer position="top-right" autoClose={3000} />
    </BrowserRouter>
  );
}

export default App;
