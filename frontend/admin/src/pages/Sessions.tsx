import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { Link } from 'react-router-dom';
import { toast } from 'react-toastify';
import { ClipboardList } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import ConfirmDialog from '../components/ui/ConfirmDialog';
import ConfirmModal from '../components/ui/ConfirmModal';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import Button from '../components/ui/Button';
import { SkeletonRows } from '../components/ui/Skeleton';

interface Location { _id: string; name: string; radiusMeters: number; }
interface Session {
  _id: string;
  locationId?: Location;
  isActive: boolean;
  expiresAt: string;
  createdAt: string;
  attendanceCount: number;
  description?: string;
}

const Sessions = () => {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [locations, setLocations] = useState<Location[]>([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [formData, setFormData] = useState({ locationId: '', durationMinutes: 30, description: '' });
  const [deleteModal, setDeleteModal] = useState({ open: false, sessionId: '', attendanceCount: 0, locationName: '' });
  const [deletePassword, setDeletePassword] = useState('');
  const [deleting, setDeleting] = useState(false);
  const [deactivateId, setDeactivateId] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const fetchData = useCallback(async () => {
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    try {
      const [sessionsRes, locationsRes] = await Promise.all([
        axios.get<Session[]>('/api/admin/sessions', { signal: abortRef.current.signal }),
        axios.get<Location[]>('/api/admin/locations', { signal: abortRef.current.signal }),
      ]);
      setSessions(sessionsRes.data);
      setLocations(locationsRes.data);
    } catch (error) {
      if ((error as { name?: string }).name !== 'CanceledError') toast.error('Failed to fetch data');
    } finally { setLoading(false); }
  }, []);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 60000);
    return () => { clearInterval(interval); abortRef.current?.abort(); };
  }, [fetchData]);

  const isExpired = (session: Session) => new Date(session.expiresAt) < new Date();

  const getStatus = (session: Session) => {
    if (!session.isActive) return { label: 'Inactive', tone: 'danger' as const };
    if (isExpired(session)) return { label: 'Expired', tone: 'warning' as const };
    return { label: 'Active', tone: 'success' as const };
  };

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    try {
      const duration = parseInt(String(formData.durationMinutes));
      if (isNaN(duration) || duration < 5 || duration > 480) { toast.error('Duration must be between 5 and 480 minutes'); return; }
      const res = await axios.post<{ _id: string }>('/api/admin/sessions', { ...formData, durationMinutes: duration });
      const slRes = await axios.post<{ shortCode: string }>('/api/admin/shortlinks', { sessionId: res.data._id });
      const { protocol, hostname } = window.location;
      const link = `${protocol}//${hostname}/s/${slRes.data.shortCode}`;
      await navigator.clipboard.writeText(link).catch(() => {});
      toast.success('Session created! Link copied to clipboard.');
      setShowModal(false);
      setFormData({ locationId: '', durationMinutes: 30, description: '' });
      fetchData();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to create session');
    }
  };

  const handleDeactivate = async (id: string) => {
    try { await axios.post(`/api/admin/sessions/${id}/deactivate`); toast.success('Session deactivated'); fetchData(); }
    catch { toast.error('Failed to deactivate session'); }
    setDeactivateId(null);
  };

  const handleDelete = async (e: FormEvent) => {
    e.preventDefault();
    if (!deletePassword) { toast.error('Please enter your admin password'); return; }
    setDeleting(true);
    try {
      await axios.delete(`/api/admin/sessions/${deleteModal.sessionId}`, { data: { password: deletePassword } });
      toast.success('Session and all attendance records deleted');
      setDeleteModal({ open: false, sessionId: '', attendanceCount: 0, locationName: '' });
      setDeletePassword('');
      fetchData();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to delete session');
    } finally { setDeleting(false); }
  };

  const sortedSessions = useMemo(() => [...sessions].sort((a, b) => new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()), [sessions]);

  const columns: Column<Session>[] = [
    { key: 'location', label: 'Location',   width: '22%', render: (s) => s.locationId?.name || 'Unknown' },
    { key: 'status',   label: 'Status',     width: '14%', render: (s) => { const st = getStatus(s); return <Badge tone={st.tone}>{st.label}</Badge>; }},
    { key: 'expires',  label: 'Expires At', width: '20%', render: (s) => new Date(s.expiresAt).toLocaleString() },
    { key: 'students', label: 'Students',   width: '12%', align: 'center', render: (s) => s.attendanceCount },
    { key: 'actions',  label: 'Actions',    width: '32%', render: (s) => (
      <div className="actions-cell">
        <Link to={`/sessions/${s._id}`} className="btn btn-secondary btn-small">View</Link>
        {s.isActive && !isExpired(s) && <Button variant="danger" size="sm" onClick={() => setDeactivateId(s._id)}>Deactivate</Button>}
        <Button variant="delete" size="sm" onClick={() => { setDeleteModal({ open: true, sessionId: s._id, attendanceCount: s.attendanceCount, locationName: s.locationId?.name || 'Unknown' }); setDeletePassword(''); }}>Delete</Button>
      </div>
    )},
  ];

  return (
    <div className="container">
      <PageHeader title="Attendance Sessions">
        <button className="btn btn-primary" onClick={() => setShowModal(true)} disabled={locations.length === 0}>Create Session</button>
      </PageHeader>

      {locations.length === 0 && (
        <div className="card"><p>No locations found. <Link to="/locations">Create a location first</Link></p></div>
      )}

      {loading ? <SkeletonRows /> : sessions.length === 0 && locations.length > 0 ? (
        <EmptyState icon={ClipboardList} title="No sessions yet" message="Create your first attendance session!" />
      ) : sessions.length > 0 ? (
        <div className="card card-table">
          <DataTable columns={columns} rows={sortedSessions} rowKey={(s) => s._id} />
        </div>
      ) : null}

      <Modal open={showModal} onClose={() => setShowModal(false)} title="Create Attendance Session">
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Location</label>
            <select value={formData.locationId} onChange={(e) => setFormData({ ...formData, locationId: e.target.value })} required>
              <option value="">Select a location</option>
              {locations.map((loc) => <option key={loc._id} value={loc._id}>{loc.name} (Radius: {loc.radiusMeters}m)</option>)}
            </select>
          </div>
          <div className="form-group">
            <label>Duration (minutes)</label>
            <input type="number" value={formData.durationMinutes} onChange={(e) => setFormData({ ...formData, durationMinutes: parseInt(e.target.value) })} min="5" max="480" required />
          </div>
          <div className="form-group">
            <label>Description (optional)</label>
            <textarea value={formData.description} onChange={(e) => setFormData({ ...formData, description: e.target.value })} rows={2} placeholder="e.g., Morning attendance for CS101" />
          </div>
          <div className="form-actions">
            <button type="submit" className="btn btn-success">Create Session</button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowModal(false)}>Cancel</button>
          </div>
        </form>
      </Modal>

      <ConfirmDialog
        open={deleteModal.open}
        onClose={() => { setDeleteModal({ open: false, sessionId: '', attendanceCount: 0, locationName: '' }); setDeletePassword(''); }}
        onSubmit={handleDelete}
        title="Delete Session"
        confirmLabel="Confirm Delete"
        loading={deleting}
        message={
          <>
            You are about to permanently delete the session for <strong>{deleteModal.locationName}</strong>.{' '}
            {deleteModal.attendanceCount > 0 ? (
              <span style={{ color: 'var(--color-danger)' }}>
                This will also delete <strong>{deleteModal.attendanceCount} attendance record{deleteModal.attendanceCount !== 1 ? 's' : ''}</strong> and all associated photos. This cannot be undone.
              </span>
            ) : <span style={{ color: 'var(--color-muted)' }}>This session has no attendance records.</span>}
          </>
        }
      >
        <div className="form-group">
          <label>Confirm with Admin Password</label>
          <input type="password" value={deletePassword} onChange={(e) => setDeletePassword(e.target.value)} placeholder="Enter your admin password" autoFocus required />
        </div>
      </ConfirmDialog>

      <ConfirmModal
        isOpen={!!deactivateId}
        title="Deactivate Session"
        message="Are you sure you want to deactivate this session? Students will no longer be able to submit attendance."
        confirmText="Deactivate"
        onConfirm={() => deactivateId && handleDeactivate(deactivateId)}
        onCancel={() => setDeactivateId(null)}
      />
    </div>
  );
};

export default Sessions;
