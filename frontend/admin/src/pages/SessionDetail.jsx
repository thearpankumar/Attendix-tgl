import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useParams, Link } from 'react-router-dom';
import axios from 'axios';
import { toast } from 'react-toastify';

const parseUA = (ua) => {
  if (!ua) return 'N/A';
  const lower = ua.toLowerCase();
  
  let os = 'Other';
  if (lower.includes('iphone')) os = 'iPhone';
  else if (lower.includes('ipad')) os = 'iPad';
  else if (lower.includes('android')) os = 'Android';
  else if (lower.includes('windows')) os = 'Windows';
  else if (lower.includes('macintosh') || lower.includes('mac os')) os = 'macOS';
  else if (lower.includes('linux')) os = 'Linux';
  
  let browser = '';
  if (lower.includes('firefox')) browser = 'Firefox';
  else if (lower.includes('chrome') || lower.includes('crios')) browser = 'Chrome';
  else if (lower.includes('safari') && !lower.includes('chrome')) browser = 'Safari';
  else if (lower.includes('edge') || lower.includes('edg')) browser = 'Edge';
  
  return browser ? `${os} (${browser})` : os;
};

const SessionDetail = () => {
  const { id } = useParams();
  const [session, setSession] = useState(null);
  const [attendance, setAttendance] = useState([]);
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);

  const fetchData = React.useCallback(async () => {
    try {
      const [sessionRes, attendanceRes, statsRes] = await Promise.all([
        axios.get(`/api/admin/sessions/${id}`),
        axios.get(`/api/admin/sessions/${id}/attendance`),
        axios.get(`/api/admin/sessions/${id}/stats`),
      ]);
      setSession(sessionRes.data);
      setAttendance(attendanceRes.data);
      setStats(statsRes.data);
    } catch (error) {
      toast.error('Failed to fetch session data');
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, [id, fetchData]);

  const handleRotateToken = async () => {
    if (!window.confirm('Rotate token? The current link will stop working.')) return;
    try {
      const res = await axios.post(`/api/admin/sessions/${id}/rotate`);
      const token = res.data.token;
      const baseUrl = window.location.origin.replace(':5173', '');
      const attendanceLink = `${baseUrl}/attend/${token}`;

      toast.success('Token rotated! New link copied to clipboard.');
      navigator.clipboard.writeText(attendanceLink);
      fetchData();
    } catch (error) {
      toast.error('Failed to rotate token');
    }
  };

  const handleDeactivate = async () => {
    if (!window.confirm('Deactivate this session?')) return;
    try {
      await axios.post(`/api/admin/sessions/${id}/deactivate`);
      toast.success('Session deactivated');
      fetchData();
    } catch (error) {
      toast.error('Failed to deactivate session');
    }
  };

  const handleExportCSV = () => {
    if (attendance.length === 0) {
      toast.error('No attendance data to export');
      return;
    }

    const headers = ['Roll Number', 'Name', 'Verified', 'Distance (m)', 'IP Address', 'Network Provider', 'Device', 'Face Detected', 'Captured At'];
    const csvContent = [
      headers.join(','),
      ...attendance.map((a) =>
        [
          a.rollNumber,
          `"${a.studentName}"`,
          a.verified ? 'Yes' : 'No',
          a.distanceFromLocation,
          a.ipAddress || 'N/A',
          `"${a.networkProvider || 'N/A'}"`,
          `"${parseUA(a.userAgent)}"`,
          a.faceDetected !== false ? 'Yes' : 'No',
          new Date(a.capturedAt).toISOString(),
        ].join(',')
      ),
    ].join('\n');

    const blob = new Blob([csvContent], { type: 'text/csv' });
    const url = window.URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `attendance-${id}-${new Date().toISOString().split('T')[0]}.csv`;
    a.click();
    window.URL.revokeObjectURL(url);
  };

  const isExpired = () => {
    return session && new Date(session.expiresAt) < new Date();
  };

  if (loading) return <div className="loading">Loading...</div>;

  if (!session) {
    return (
      <div className="container">
        <p>Session not found</p>
        <Link to="/sessions" className="btn btn-secondary">
          Back to Sessions
        </Link>
      </div>
    );
  }

  return (
    <div className="container">
      <div className="row">
        <Link to="/sessions" className="btn btn-secondary btn-small">
          &larr; Back
        </Link>
      </div>

      <h2 style={{ margin: '20px 0' }}>Session Details</h2>

      <div className="card">
        <h3>Session Info</h3>
        <p>
          <strong>Location:</strong> {session.locationId?.name || 'Unknown'}
        </p>
        <p>
          <strong>Status:</strong>{' '}
          <span
            className={`badge ${
              !session.isActive
                ? 'badge-danger'
                : isExpired()
                ? 'badge-warning'
                : 'badge-success'
            }`}
          >
            {!session.isActive
              ? 'Inactive'
              : isExpired()
              ? 'Expired'
              : 'Active'}
          </span>
        </p>
        <p>
          <strong>Expires At:</strong> {new Date(session.expiresAt).toLocaleString()}
        </p>
        <p>
          <strong>Rotations:</strong> {session.rotationCount}
        </p>

        <div style={{ marginTop: '20px' }}>
          <button
            className="btn btn-primary"
            onClick={handleRotateToken}
            disabled={!session.isActive}
          >
            Rotate Token
          </button>
          {session.totpEnabled && (
            <Link
              to={`/sessions/${id}/qr`}
              className="btn btn-success"
              style={{ marginLeft: '10px' }}
            >
              📱 View QR Display
            </Link>
          )}
          {session.isActive && !isExpired() && (
            <button
              className="btn btn-danger"
              onClick={handleDeactivate}
              style={{ marginLeft: '10px' }}
            >
              Deactivate Session
            </button>
          )}
        </div>
      </div>

      <div className="grid">
        <div className="stat-card">
          <h3>Total Attendance</h3>
          <p>{stats?.totalAttendance || 0}</p>
        </div>
        <div className="stat-card">
          <h3>Verified</h3>
          <p style={{ color: '#28a745' }}>{stats?.verifiedAttendance || 0}</p>
        </div>
        <div className="stat-card">
          <h3>Unverified</h3>
          <p style={{ color: '#dc3545' }}>
            {(stats?.totalAttendance || 0) - (stats?.verifiedAttendance || 0)}
          </p>
        </div>
      </div>

      <div className="card">
        <div className="row">
          <h3>Attendance Records</h3>
          <button className="btn btn-secondary btn-small" onClick={handleExportCSV}>
            Export CSV
          </button>
        </div>

        {(() => {
          const ipCounts = {};
          attendance.forEach((a) => {
            if (a.ipAddress) {
              ipCounts[a.ipAddress] = (ipCounts[a.ipAddress] || 0) + 1;
            }
          });

          return attendance.length === 0 ? (
            <p>No attendance records yet</p>
          ) : (
            <table className="table">
              <thead>
                <tr>
                  <th>Roll No.</th>
                  <th>Name</th>
                  <th>Photo</th>
                  <th>Distance</th>
                  <th>IP Address</th>
                  <th>Device</th>
                  <th>Face Check</th>
                  <th>Status</th>
                  <th>Time</th>
                </tr>
              </thead>
              <tbody>
                {attendance.map((a) => (
                  <tr key={a._id}>
                    <td>{a.rollNumber}</td>
                    <td>{a.studentName}</td>
                    <td>
                      <img
                        src={a.photoUrl}
                        alt="Student"
                        style={{
                          width: '50px',
                          height: '50px',
                          objectFit: 'cover',
                          borderRadius: '4px',
                        }}
                      />
                    </td>
                    <td>{a.distanceFromLocation}m</td>
                    <td>
                      <div style={{ fontWeight: '500' }}>{a.ipAddress || 'N/A'}</div>
                      {a.networkProvider && (
                        <div style={{ fontSize: '11px', color: '#666', marginTop: '2px', wordBreak: 'break-word', maxWidth: '180px' }}>
                          {a.networkProvider}
                        </div>
                      )}
                      {a.ipAddress && ipCounts[a.ipAddress] > 1 && (
                        <span
                          className="badge badge-danger"
                          style={{
                            fontSize: '9px',
                            padding: '2px 4px',
                            marginTop: '4px',
                            display: 'inline-block',
                            backgroundColor: '#dc3545',
                            color: 'white',
                          }}
                        >
                          ⚠️ Shared ({ipCounts[a.ipAddress]})
                        </span>
                      )}
                    </td>
                    <td style={{ fontSize: '12px' }} title={a.userAgent}>
                      {parseUA(a.userAgent)}
                    </td>
                    <td>
                      <span
                        className={`badge ${
                          a.faceDetected !== false ? 'badge-success' : 'badge-danger'
                        }`}
                        style={{
                          fontSize: '11px',
                          padding: '3px 6px',
                        }}
                      >
                        {a.faceDetected !== false ? 'Detected' : 'No Face'}
                      </span>
                    </td>
                    <td>
                      <span
                        className={`badge ${
                          a.verified ? 'badge-success' : 'badge-danger'
                        }`}
                      >
                        {a.verified ? 'Verified' : 'Unverified'}
                      </span>
                    </td>
                    <td>{new Date(a.capturedAt).toLocaleTimeString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          );
        })()}
      </div>
    </div>
  );
};

export default SessionDetail;
