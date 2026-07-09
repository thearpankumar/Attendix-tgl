import React from 'react';
import { Link } from 'react-router-dom';

const NotFound: React.FC = () => {
  return (
    <div style={{ textAlign: 'center', padding: '50px', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
      <h1>404 - Page Not Found</h1>
      <p>The page you are looking for does not exist.</p>
      <Link to="/" className="btn btn-primary" style={{ marginTop: '20px' }}>Go to Dashboard</Link>
    </div>
  );
};

export default NotFound;
