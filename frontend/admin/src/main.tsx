import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import 'react-toastify/dist/ReactToastify.css';
import { AuthProvider } from './context/AuthContext';

// Short-link / student paths belong to the Caddy front door, not the admin's
// dev port (:3000). If one lands here, bounce to the same path without the port
// so it hits Caddy (:80). Harmless in prod, where these never reach this app.
{
  const { pathname, search, protocol, hostname, port } = window.location;
  if (import.meta.env.DEV && port && (pathname.startsWith('/s/') || pathname.startsWith('/attend/'))) {
    window.location.replace(`${protocol}//${hostname}${pathname}${search}`);
  }
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <AuthProvider>
      <App />
    </AuthProvider>
  </React.StrictMode>
);
