import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { ToastContainer } from 'react-toastify';
import { useAuth } from './context/AuthContext';
import Login from './pages/Login';
import Dashboard from './pages/Dashboard';
import Locations from './pages/Locations';
import Sessions from './pages/Sessions';
import SessionDetail from './pages/SessionDetail';
import Navbar from './components/Navbar';

const PrivateRoute = ({ children }) => {
  const { admin, loading } = useAuth();
  
  if (loading) {
    return <div className="loading">Loading...</div>;
  }
  
  return admin ? children : <Navigate to="/login" />;
};

function App() {
  const { admin } = useAuth();

  return (
    <BrowserRouter basename="/admin">
      {admin && <Navbar />}
      <Routes>
        <Route path="/login" element={admin ? <Navigate to="/" /> : <Login />} />
        <Route
          path="/"
          element={
            <PrivateRoute>
              <Dashboard />
            </PrivateRoute>
          }
        />
        <Route
          path="/locations"
          element={
            <PrivateRoute>
              <Locations />
            </PrivateRoute>
          }
        />
        <Route
          path="/sessions"
          element={
            <PrivateRoute>
              <Sessions />
            </PrivateRoute>
          }
        />
        <Route
          path="/sessions/:id"
          element={
            <PrivateRoute>
              <SessionDetail />
            </PrivateRoute>
          }
        />
      </Routes>
      <ToastContainer position="top-right" autoClose={3000} />
    </BrowserRouter>
  );
}

export default App;
