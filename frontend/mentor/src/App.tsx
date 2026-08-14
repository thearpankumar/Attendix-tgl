import { BrowserRouter, Routes, Route, Navigate } from 'react-router';
import { ToastContainer } from 'react-toastify';
import type { ReactNode } from 'react';
import { useAuth } from './context/AuthContext';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import SessionDetail from './pages/SessionDetail';
import Settings from './pages/Settings';
import NotFound from './pages/NotFound';
import Topbar from './components/Topbar';

const PrivateRoute = ({ children }: { children: ReactNode }) => {
  const { admin, loading } = useAuth();
  if (loading) return <div className="loading-screen">Loading…</div>;
  if (!admin) return <Navigate to="/login" />;
  return (
    <div className="app-shell">
      <Topbar />
      <main className="page">{children}</main>
    </div>
  );
};

function App() {
  const { admin } = useAuth();

  return (
    <BrowserRouter basename={import.meta.env.BASE_URL.replace(/\/$/, '')}>
      <Routes>
        <Route path="/login" element={admin ? <Navigate to="/" /> : <Login />} />
        <Route path="/" element={<PrivateRoute><Dashboard /></PrivateRoute>} />
        <Route path="/sessions/:id" element={<PrivateRoute><SessionDetail /></PrivateRoute>} />
        <Route path="/settings" element={<PrivateRoute><Settings /></PrivateRoute>} />
        <Route path="*" element={<NotFound />} />
      </Routes>
      <ToastContainer position="top-center" autoClose={2500} />
    </BrowserRouter>
  );
}

export default App;
