import React, { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { Copy, Link2 } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import { SkeletonRows } from '../components/ui/Skeleton';

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

  const activeSessions = sessions.filter((s) => s.isActive && new Date(s.expiresAt) > new Date());

  const columns = [
    {
      key: 'shortCode',
      label: 'Short Code',
      priority: 1,
      render: (link) => (
        <code style={{ fontSize: '13px', background: '#f4f4f4', padding: '4px 8px', borderRadius: '4px' }}>
          {link.shortCode}
        </code>
      ),
    },
    {
      key: 'url',
      label: 'Full URL',
      priority: 2,
      render: (link) => {
        const fullUrl = getFullUrl(link.shortCode);
        return (
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <a href={fullUrl} target="_blank" rel="noopener noreferrer" style={{ fontSize: '13px' }}>
              {fullUrl.length > 35 ? fullUrl.substring(0, 35) + '...' : fullUrl}
            </a>
            <button onClick={() => copyToClipboard(fullUrl)} className="btn btn-small" aria-label="Copy link">
              <Copy size={12} />
            </button>
          </div>
        );
      },
    },
    {
      key: 'session',
      label: 'Attached Session',
      priority: 2,
      render: (link) =>
        link.sessionId ? (
          <div>
            <div style={{ fontWeight: 500 }}>{link.sessionId.description || 'Session'}</div>
            <div style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>
              Expires: {new Date(link.sessionId.expiresAt).toLocaleString()}
            </div>
          </div>
        ) : (
          <span style={{ color: 'var(--color-text-faint)' }}>Not attached</span>
        ),
    },
    {
      key: 'status',
      label: 'Status',
      priority: 1,
      render: (link) => {
        const isActive = link.isActive && link.sessionId && link.sessionId.isActive;
        return <Badge tone={isActive ? 'success' : 'warning'}>{isActive ? 'Active' : 'Inactive'}</Badge>;
      },
    },
    { key: 'clicks', label: 'Clicks', priority: 3, render: (link) => link.clickCount || 0 },
    {
      key: 'created',
      label: 'Created',
      priority: 3,
      render: (link) => new Date(link.createdAt).toLocaleDateString(),
    },
    {
      key: 'actions',
      label: 'Actions',
      priority: 1,
      render: (link) => (
        <div className="actions-cell">
          {link.sessionId ? (
            <button className="btn btn-secondary btn-small" onClick={() => handleDetach(link.shortCode)}>
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
              <option value="" disabled>
                Attach to...
              </option>
              {activeSessions.map((s) => (
                <option key={s._id} value={s._id}>
                  {s.description || s.locationId?.name || 'Session'}
                </option>
              ))}
            </select>
          )}
          <button className="btn btn-delete btn-small" onClick={() => handleDelete(link.shortCode)}>
            Delete
          </button>
        </div>
      ),
    },
  ];

  return (
    <div className="container">
      <PageHeader title="Short Links Management">
        <button className="btn btn-primary" onClick={() => setShowModal(true)}>
          Create Short Link
        </button>
      </PageHeader>

      <div className="info-card">
        <h4>How it works</h4>
        <ul>
          <li>
            <strong>Create</strong> a short link (e.g., <code>myclass123</code>)
          </li>
          <li>
            <strong>Attach</strong> it to an active attendance session
          </li>
          <li>
            Share the link: <code>yourdomain.com/s/myclass123</code>
          </li>
          <li>
            Students see a <strong>time-rotating QR code</strong> that changes every 5 seconds
          </li>
          <li>Students must scan the current code to mark attendance</li>
        </ul>
      </div>

      {loading ? (
        <SkeletonRows />
      ) : shortLinks.length === 0 ? (
        <EmptyState
          icon={Link2}
          title="No short links yet"
          message="Create your first short link to start sharing attendance sessions."
        />
      ) : (
        <div className="card">
          <DataTable columns={columns} rows={shortLinks} rowKey={(l) => l._id} />
        </div>
      )}

      <Modal open={showModal} onClose={() => setShowModal(false)} title="Create Short Link">
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <input
                type="checkbox"
                style={{ width: 'auto', minHeight: 'unset' }}
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
              <small className="form-hint">3-20 characters: lowercase letters, numbers, hyphens only</small>
            </div>
          )}

          <div className="form-group">
            <label>Attach to Session (optional)</label>
            <select
              value={formData.sessionId}
              onChange={(e) => setFormData({ ...formData, sessionId: e.target.value })}
            >
              <option value="">Create without attaching</option>
              {activeSessions.map((s) => (
                <option key={s._id} value={s._id}>
                  {s.description || 'Session'} - {s.locationId?.name || 'Unknown'}
                </option>
              ))}
            </select>
          </div>

          <div className="form-actions">
            <button type="submit" className="btn btn-success">
              Create Short Link
            </button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowModal(false)}>
              Cancel
            </button>
          </div>
        </form>
      </Modal>
    </div>
  );
};

export default ShortLinks;
