import React, { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';

const ShortLinks = () => {
  const [shortLinks, setShortLinks] = useState([]);
  const [sessions, setSessions] = useState([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [formData, setFormData] = useState({
    shortCode: '',
    sessionId: '',
  });
  const [autoGenerate, setAutoGenerate] = useState(true);

  const fetchData = useCallback(async () => {
    try {
      const [linksRes, sessionsRes] = await Promise.all([
        axios.get('/api/admin/shortlinks'),
        axios.get('/api/admin/sessions'),
      ]);
      setShortLinks(linksRes.data.shortLinks);
      setSessions(sessionsRes.data);
    } catch (error) {
      toast.error('Failed to fetch data');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleSubmit = async (e) => {
    e.preventDefault();
    try {
      const payload = {
        shortCode: autoGenerate ? '' : formData.shortCode.trim().toLowerCase(),
        sessionId: formData.sessionId || null,
      };
      
      const res = await axios.post('/api/admin/shortlinks', payload);
      toast.success(`Short link created: ${res.data.shortCode}`);
      setShowModal(false);
      setFormData({ shortCode: '', sessionId: '' });
      fetchData();
    } catch (error) {
      toast.error(error.response?.data?.message || 'Failed to create short link');
    }
  };

  const handleAttach = async (shortCode, sessionId) => {
    try {
      await axios.post(`/api/admin/shortlinks/${shortCode}/attach`, { sessionId });
      toast.success('Short link attached to session');
      fetchData();
    } catch (error) {
      toast.error(error.response?.data?.message || 'Failed to attach');
    }
  };

  const handleDetach = async (shortCode) => {
    if (!window.confirm('Detach this short link from the session?')) return;
    try {
      await axios.post(`/api/admin/shortlinks/${shortCode}/detach`);
      toast.success('Short link detached');
      fetchData();
    } catch (error) {
      toast.error('Failed to detach');
    }
  };

  const handleDelete = async (shortCode) => {
    if (!window.confirm('Delete this short link permanently?')) return;
    try {
      await axios.delete(`/api/admin/shortlinks/${shortCode}`);
      toast.success('Short link deleted');
      fetchData();
    } catch (error) {
      toast.error('Failed to delete');
    }
  };

  const copyToClipboard = (text) => {
    navigator.clipboard.writeText(text);
    toast.success('Link copied to clipboard');
  };

  const getFullUrl = (shortCode) => {
    const protocol = window.location.protocol;
    const hostname = window.location.hostname;
    const port = window.location.port;
    
    if (hostname === 'localhost' || hostname === '127.0.0.1') {
      return `${protocol}//${hostname}:${port || 80}/s/${shortCode}`;
    }
    
    return `${protocol}//${hostname}/s/${shortCode}`;
  };

  const activeSessions = sessions.filter(s => s.isActive && new Date(s.expiresAt) > new Date());

  if (loading) return <div className="loading">Loading...</div>;

  return (
    <div className="container">
      <div className="row">
        <h2>Short Links Management</h2>
        <button
          className="btn btn-primary"
          onClick={() => setShowModal(true)}
        >
          Create Short Link
        </button>
      </div>

      <div className="card" style={{ marginTop: '20px', marginBottom: '20px', background: '#f0f8ff', border: '1px solid #b3d9ff' }}>
        <h4 style={{ marginBottom: '10px' }}>How it works:</h4>
        <ul style={{ marginLeft: '20px', lineHeight: '1.8' }}>
          <li><strong>Create</strong> a short link (e.g., <code>myclass123</code>)</li>
          <li><strong>Attach</strong> it to an active attendance session</li>
          <li>Share the link: <code>yourdomain.com/s/myclass123</code></li>
          <li>Students see a <strong>time-rotating QR code</strong> that changes every 5 seconds</li>
          <li>Students must scan the <strong>current</strong> code to mark attendance</li>
        </ul>
      </div>

      {shortLinks.length === 0 ? (
        <div className="card">
          <p>No short links created yet. Create your first short link!</p>
        </div>
      ) : (
        <div className="card">
          <table className="table">
            <thead>
              <tr>
                <th>Short Code</th>
                <th>Full URL</th>
                <th>Attached Session</th>
                <th>Status</th>
                <th>Clicks</th>
                <th>Created</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {shortLinks.map((link) => {
                const fullUrl = getFullUrl(link.shortCode);
                const attachedSession = link.sessionId;
                const isActive = link.isActive && attachedSession && attachedSession.isActive;
                
                return (
                  <tr key={link._id}>
                    <td>
                      <code style={{ fontSize: '14px', background: '#f4f4f4', padding: '4px 8px', borderRadius: '4px' }}>
                        {link.shortCode}
                      </code>
                    </td>
                    <td>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <a 
                          href={fullUrl} 
                          target="_blank" 
                          rel="noopener noreferrer"
                          style={{ color: '#667eea', fontSize: '13px' }}
                        >
                          {fullUrl.length > 35 ? fullUrl.substring(0, 35) + '...' : fullUrl}
                        </a>
                        <button
                          onClick={() => copyToClipboard(fullUrl)}
                          className="btn btn-small"
                          style={{ padding: '4px 8px', fontSize: '12px' }}
                        >
                          📋
                        </button>
                      </div>
                    </td>
                    <td>
                      {attachedSession ? (
                        <div>
                          <div style={{ fontWeight: '500' }}>
                            {attachedSession.description || 'Session'}
                          </div>
                          <div style={{ fontSize: '12px', color: '#666' }}>
                            Expires: {new Date(attachedSession.expiresAt).toLocaleString()}
                          </div>
                        </div>
                      ) : (
                        <span style={{ color: '#999' }}>Not attached</span>
                      )}
                    </td>
                    <td>
                      <span className={`badge ${isActive ? 'badge-success' : 'badge-warning'}`}>
                        {isActive ? 'Active' : 'Inactive'}
                      </span>
                    </td>
                    <td>{link.clickCount || 0}</td>
                    <td style={{ fontSize: '13px' }}>
                      {new Date(link.createdAt).toLocaleDateString()}
                    </td>
                    <td>
                      <div className="actions-cell">
                        {attachedSession ? (
                          <button
                            className="btn btn-secondary btn-small"
                            onClick={() => handleDetach(link.shortCode)}
                          >
                            Detach
                          </button>
                        ) : (
                          <select
                            className="btn btn-small"
                            style={{ padding: '4px 8px' }}
                            onChange={(e) => {
                              if (e.target.value) {
                                handleAttach(link.shortCode, e.target.value);
                                e.target.value = '';
                              }
                            }}
                            defaultValue=""
                          >
                            <option value="" disabled>Attach to...</option>
                            {activeSessions.map((s) => (
                              <option key={s._id} value={s._id}>
                                {s.description || s.locationId?.name || 'Session'} 
                              </option>
                            ))}
                          </select>
                        )}
                        <button
                          className="btn btn-delete btn-small"
                          onClick={() => handleDelete(link.shortCode)}
                        >
                          Delete
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {showModal && (
        <div className="modal-overlay" onClick={() => setShowModal(false)}>
          <div className="modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="modal-header">
              <h3>Create Short Link</h3>
              <button className="modal-close" onClick={() => setShowModal(false)}>
                &times;
              </button>
            </div>
            <form onSubmit={handleSubmit}>
              <div className="form-group">
                <label style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                  <input
                    type="checkbox"
                    checked={autoGenerate}
                    onChange={(e) => setAutoGenerate(e.target.checked)}
                  />
                  Auto-generate short code
                </label>
              </div>
              
              {!autoGenerate && (
                <div className="form-group">
                  <label>Custom Short Code</label>
                  <input
                    type="text"
                    value={formData.shortCode}
                    onChange={(e) => setFormData({ ...formData, shortCode: e.target.value })}
                    placeholder="e.g., cs101-monday"
                    pattern="[a-z0-9-]{3,20}"
                    required
                  />
                  <small style={{ color: '#666', fontSize: '12px' }}>
                    3-20 characters: lowercase letters, numbers, hyphens only
                  </small>
                </div>
              )}
              
              <div className="form-group">
                <label>Attach to Session (optional)</label>
                <select
                  value={formData.sessionId}
                  onChange={(e) => setFormData({ ...formData, sessionId: e.target.value })}
                  style={{
                    width: '100%',
                    padding: '10px',
                    border: '1px solid #ddd',
                    borderRadius: '4px',
                  }}
                >
                  <option value="">Create without attaching</option>
                  {activeSessions.map((s) => (
                    <option key={s._id} value={s._id}>
                      {s.description || 'Session'} - {s.locationId?.name || 'Unknown'}
                    </option>
                  ))}
                </select>
              </div>
              
              <div style={{ display: 'flex', gap: '10px', marginTop: '20px' }}>
                <button type="submit" className="btn btn-success">
                  Create Short Link
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => setShowModal(false)}
                >
                  Cancel
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};

export default ShortLinks;
