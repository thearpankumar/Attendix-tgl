import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';
import 'react-toastify/dist/ReactToastify.css';
import { AuthProvider } from './context/AuthContext';
import { ErrorBoundary } from './components/ErrorBoundary';
import axios from 'axios';

// Configure Axios globally to send cookies (for admin_token) and automatically
// attach the CSRF token to mutating requests.
axios.defaults.withCredentials = true;
axios.defaults.xsrfCookieName = 'csrf_token';
axios.defaults.xsrfHeaderName = 'x-csrf-token';
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
    <ErrorBoundary appName="AdminFrontend">
      <AuthProvider>
        <App />
      </AuthProvider>
    </ErrorBoundary>
  </React.StrictMode>
);
