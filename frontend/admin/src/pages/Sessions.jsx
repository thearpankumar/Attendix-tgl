import React, { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import axios from 'axios';
import { Link } from 'react-router-dom';
import { toast } from 'react-toastify';
import { ClipboardList } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import ConfirmDialog from '../components/ui/ConfirmDialog';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import { SkeletonRows } from '../components/ui/Skeleton';

const Sessions = () => {
  const [sessions, setSessions] = useState([]);
  const [locations, setLocations] = useState([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [formData, setFormData] = useState({
    locationId: '',
    durationMinutes: 30,
    description: '',
  });
  const [deleteModal, setDeleteModal] = useState({ open: false, sessionId: null, attendanceCount: 0, locationName: '' });
  const [deletePassword, setDeletePassword] = useState('');
  const [deleting, setDeleting] = useState(false);
  const abortControllerRef = useRef(null);

  const fetchData = useCallback(async () => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    abortControllerRef.current = new AbortController();

    try {
      const [sessionsRes, locationsRes] = await Promise.all([
        axios.get('/api/admin/sessions', { signal: abortControllerRef.current.signal }),
        axios.get('/api/admin/locations', { signal: abortControllerRef.current.signal }),
      ]);
      setSessions(sessionsRes.data);
      setLocations(locationsRes.data);
    } catch (error) {
      if (error.name !== 'CanceledError') {
        toast.error('Failed to fetch data');
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 60000);
    return () => {
      clearInterval(interval);
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
      }
    };
  }, [fetchData]);

  const getAttendanceLink = (shortCode) => {
    const { protocol, hostname } = window.location;
    return `${protocol}//${hostname}/s/${shortCode}`;
  };

  const handleSubmit = async (e) => {
    e.preventDefault();
    try {
      const duration = parseInt(formData.durationMinutes);
      if (isNaN(duration) || duration < 5 || duration > 480) {
        toast.error('Duration must be between 5 and 480 minutes');
        return;
      }

      const res = await axios.post('/api/admin/sessions', {
        ...formData,
        durationMinutes: duration,
      });

      const sessionId = res.data._id;
      const slRes = await axios.post('/api/admin/shortlinks', { sessionId });
      const attendanceLink = getAttendanceLink(slRes.data.shortCode);

      await navigator.clipboard.writeText(attendanceLink).catch(() => {});
      toast.success('Session created! Link copied to clipboard.');

      setShowModal(false);
      setFormData({ locationId: '', durationMinutes: 30, description: '' });
      fetchData();
    } catch (error) {
      toast.error(error.response?.data?.message || 'Failed to create session');
    }
  };

  const handleDeactivate = async (id) => {
    if (!window.confirm('Deactivate this session?')) return;
    try {
      await axios.post(`/api/admin/sessions/${id}/deactivate`);
      toast.success('Session deactivated');
      fetchData();
    } catch (error) {
      toast.error('Failed to deactivate session');
    }
  };

  const openDeleteModal = (session) => {
    setDeleteModal({
      open: true,
      sessionId: session._id,
      attendanceCount: session.attendanceCount,
      locationName: session.locationId?.name || 'Unknown',
    });
    setDeletePassword('');
  };

  const closeDeleteModal = () => {
    setDeleteModal({ open: false, sessionId: null, attendanceCount: 0, locationName: '' });
    setDeletePassword('');
  };

  const handleDelete = async (e) => {
    e.preventDefault();
    if (!deletePassword) {
      toast.error('Please enter your admin password');
      return;
    }
    setDeleting(true);
    try {
      await axios.delete(`/api/admin/sessions/${deleteModal.sessionId}`, {
        data: { password: deletePassword },
      });
      toast.success('Session and all attendance records deleted');
      closeDeleteModal();
      fetchData();
    } catch (error) {
      toast.error(error.response?.data?.message || 'Failed to delete session');
    } finally {
      setDeleting(false);
    }
  };

  const isExpired = useCallback((session) => {
    return new Date(session.expiresAt) < new Date();
  }, []);

  const getStatus = useCallback(
    (session) => {
      if (!session.isActive) return { label: 'Inactive', tone: 'danger' };
      if (isExpired(session)) return { label: 'Expired', tone: 'warning' };
      return { label: 'Active', tone: 'success' };
    },
    [isExpired]
  );

  const sortedSessions = useMemo(() => {
    return [...sessions].sort((a, b) => new Date(b.createdAt) - new Date(a.createdAt));
  }, [sessions]);

  const columns = [
    { key: 'location', label: 'Location', priority: 1, render: (s) => s.locationId?.name || 'Unknown' },
    {
      key: 'status',
      label: 'Status',
      priority: 1,
      render: (s) => {
        const status = getStatus(s);
        return <Badge tone={status.tone}>{status.label}</Badge>;
      },
    },
    {
      key: 'expires',
      label: 'Expires At',
      priority: 2,
      render: (s) => new Date(s.expiresAt).toLocaleString(),
    },
    { key: 'students', label: 'Students', priority: 2, render: (s) => s.attendanceCount },
    {
      key: 'actions',
      label: 'Actions',
      priority: 1,
      render: (s) => (
        <div className="actions-cell">
          <Link to={`/sessions/${s._id}`} className="btn btn-secondary btn-small">
            View
          </Link>
          {s.isActive && !isExpired(s) && (
            <button className="btn btn-danger btn-small" onClick={() => handleDeactivate(s._id)}>
              Deactivate
            </button>
          )}
          <button className="btn btn-delete btn-small" onClick={() => openDeleteModal(s)}>
            Delete
          </button>
        </div>
      ),
    },
  ];

  return (
    <div className="container">
      <PageHeader title="Attendance Sessions">
        <button className="btn btn-primary" onClick={() => setShowModal(true)} disabled={locations.length === 0}>
          Create Session
        </button>
      </PageHeader>

      {locations.length === 0 && (
        <div className="card">
          <p>
            No locations found. <Link to="/locations">Create a location first</Link>
          </p>
        </div>
      )}

      {loading ? (
        <SkeletonRows />
      ) : sessions.length === 0 && locations.length > 0 ? (
        <EmptyState icon={ClipboardList} title="No sessions yet" message="Create your first attendance session!" />
      ) : sessions.length > 0 ? (
        <div className="card">
          <DataTable columns={columns} rows={sortedSessions} rowKey={(s) => s._id} />
        </div>
      ) : null}

      <Modal open={showModal} onClose={() => setShowModal(false)} title="Create Attendance Session">
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Location</label>
            <select
              value={formData.locationId}
              onChange={(e) => setFormData({ ...formData, locationId: e.target.value })}
              required
            >
              <option value="">Select a location</option>
              {locations.map((loc) => (
                <option key={loc._id} value={loc._id}>
                  {loc.name} (Radius: {loc.radiusMeters}m)
                </option>
              ))}
            </select>
          </div>
          <div className="form-group">
            <label>Duration (minutes)</label>
            <input
              type="number"
              value={formData.durationMinutes}
              onChange={(e) => setFormData({ ...formData, durationMinutes: e.target.value })}
              min="5"
              max="480"
              required
            />
          </div>
          <div className="form-group">
            <label>Description (optional)</label>
            <textarea
              value={formData.description}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              rows="2"
              placeholder="e.g., Morning attendance for CS101"
            />
          </div>
          <div className="form-actions">
            <button type="submit" className="btn btn-success">
              Create Session
            </button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowModal(false)}>
              Cancel
            </button>
          </div>
        </form>
      </Modal>

      <ConfirmDialog
        open={deleteModal.open}
        onClose={closeDeleteModal}
        onSubmit={handleDelete}
        title="Delete Session"
        confirmLabel="Confirm Delete"
        loading={deleting}
        message={
          <>
            You are about to permanently delete the session for <strong>{deleteModal.locationName}</strong>.
            <br />
            {deleteModal.attendanceCount > 0 ? (
              <span style={{ color: 'var(--color-danger-strong)' }}>
                This will also delete{' '}
                <strong>
                  {deleteModal.attendanceCount} attendance record{deleteModal.attendanceCount !== 1 ? 's' : ''}
                </strong>{' '}
                and all associated photos from Cloudinary. This cannot be undone.
              </span>
            ) : (
              <span style={{ color: 'var(--color-text-muted)' }}>
                This session has no attendance records. It will be permanently deleted.
              </span>
            )}
          </>
        }
      >
        <div className="form-group">
          <label>Confirm with Admin Password</label>
          <input
            type="password"
            value={deletePassword}
            onChange={(e) => setDeletePassword(e.target.value)}
            placeholder="Enter your admin password"
            autoFocus
            required
          />
        </div>
      </ConfirmDialog>
    </div>
  );
};

export default Sessions;
