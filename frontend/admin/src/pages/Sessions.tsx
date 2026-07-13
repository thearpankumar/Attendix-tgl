import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { Link } from 'react-router-dom';
import { toast } from 'react-toastify';
import { ClipboardList, Sparkles, Link as LinkIcon, Pencil } from 'lucide-react';
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
interface ShortLink { _id: string; shortCode: string; isActive: boolean; sessionId?: unknown; }
type ShortlinkMode = 'auto' | 'custom' | 'existing';
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
  const [formData, setFormData] = useState({
    locationId: '',
    durationMinutes: 30,
    description: '',
    shortlinkMode: 'auto' as ShortlinkMode,
    customShortCode: '',
    existingShortCode: '',
  });
  const [freeShortLinks, setFreeShortLinks] = useState<ShortLink[]>([]);
  const [deleteModal, setDeleteModal] = useState({ open: false, sessionId: '', attendanceCount: 0, locationName: '' });
  const [deletePassword, setDeletePassword] = useState('');
  const [deleting, setDeleting] = useState(false);
  const [deactivateId, setDeactivateId] = useState<string | null>(null);
  const abortRef = useRef<AbortController | null>(null);

  const fetchData = useCallback(async () => {
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    try {
      const [sessionsRes, locationsRes, shortLinksRes] = await Promise.all([
        axios.get<Session[]>('/api/admin/sessions', { signal: abortRef.current.signal }),
        axios.get<Location[]>('/api/admin/locations', { signal: abortRef.current.signal }),
        axios.get<{ shortLinks: ShortLink[] }>('/api/admin/shortlinks', { signal: abortRef.current.signal }),
      ]);
      setSessions(sessionsRes.data);
      setLocations(locationsRes.data);
      setFreeShortLinks((shortLinksRes.data.shortLinks ?? []).filter((l) => !l.sessionId && l.isActive));
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
      if (formData.shortlinkMode === 'existing' && !formData.existingShortCode) {
        toast.error('Please select an existing short link, or switch to a different mode.');
        return;
      }
      const res = await axios.post<{ _id: string }>('/api/admin/sessions', {
        locationId: formData.locationId,
        durationMinutes: duration,
        description: formData.description,
      });

      const sessionId = res.data._id;
      const { protocol, hostname } = window.location;
      let successMessage = 'Session created successfully!';

      if (formData.shortlinkMode === 'auto') {
        const slRes = await axios.post<{ shortCode: string }>('/api/admin/shortlinks', { sessionId });
        const link = `${protocol}//${hostname}/s/${slRes.data.shortCode}`;
        await navigator.clipboard.writeText(link).catch(() => {});
        successMessage = `Session created! Link (/s/${slRes.data.shortCode}) copied to clipboard.`;
      } else if (formData.shortlinkMode === 'custom' && formData.customShortCode.trim()) {
        const slRes = await axios.post<{ shortCode: string }>('/api/admin/shortlinks', { sessionId, shortCode: formData.customShortCode.trim() });
        const link = `${protocol}//${hostname}/s/${slRes.data.shortCode}`;
        await navigator.clipboard.writeText(link).catch(() => {});
        successMessage = `Session created! Link (/s/${slRes.data.shortCode}) copied to clipboard.`;
      } else if (formData.shortlinkMode === 'existing' && formData.existingShortCode) {
        await axios.post(`/api/admin/shortlinks/${formData.existingShortCode}/attach`, { sessionId });
        const link = `${protocol}//${hostname}/s/${formData.existingShortCode}`;
        await navigator.clipboard.writeText(link).catch(() => {});
        successMessage = `Session created! Link (/s/${formData.existingShortCode}) copied to clipboard.`;
      }

      toast.success(successMessage);
      setShowModal(false);
      setFormData({ locationId: '', durationMinutes: 30, description: '', shortlinkMode: 'auto', customShortCode: '', existingShortCode: '' });
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
            <label htmlFor="session-location">Location</label>
            <select id="session-location" value={formData.locationId} onChange={(e) => setFormData({ ...formData, locationId: e.target.value })} required>
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
          {/* ── Short Link mode selector ── */}
          <div className="form-group" style={{ marginTop: '1rem' }}>
            <label>Short Link</label>
            <div style={{ display: 'flex', gap: '0.4rem', marginTop: '0.375rem' }}>
              {(['auto', 'existing', 'custom'] as ShortlinkMode[]).map((mode) => (
                <label key={mode} style={{
                  flex: 1, textAlign: 'center', padding: '0.45rem 0.25rem',
                  borderRadius: '6px', cursor: 'pointer', fontSize: '0.78rem', fontWeight: 500,
                  border: `1.5px solid ${
                    formData.shortlinkMode === mode ? 'var(--color-primary, #4f46e5)' : 'var(--color-border, #e5e7eb)'
                  }`,
                  background: formData.shortlinkMode === mode ? 'var(--color-primary-subtle, #eef2ff)' : 'transparent',
                  color: formData.shortlinkMode === mode ? 'var(--color-primary, #4f46e5)' : 'var(--color-muted)',
                  transition: 'all 0.15s',
                  userSelect: 'none',
                }}>
                  <input type="radio" name="shortlinkMode" value={mode}
                    checked={formData.shortlinkMode === mode}
                    onChange={() => setFormData({ ...formData, shortlinkMode: mode })}
                    style={{ display: 'none' }} />
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '6px' }}>
                    {mode === 'auto' && <Sparkles size={14} />}
                    {mode === 'existing' && <LinkIcon size={14} />}
                    {mode === 'custom' && <Pencil size={14} />}
                    <span>{mode === 'auto' ? 'Auto-generate' : mode === 'existing' ? 'Attach existing' : 'Custom code'}</span>
                  </div>
                </label>
              ))}
            </div>
          </div>

          {formData.shortlinkMode === 'auto' && (
            <p style={{ fontSize: '0.8rem', color: 'var(--color-muted)', marginTop: '-0.25rem', marginBottom: '0.5rem' }}>
              A random 6-character link will be created and copied to your clipboard.
            </p>
          )}

          {formData.shortlinkMode === 'existing' && (
            <div className="form-group">
              {freeShortLinks.length === 0 ? (
                <div style={{
                  padding: '0.75rem', borderRadius: '6px', fontSize: '0.82rem',
                  background: 'var(--color-warning-subtle, #fffbeb)',
                  border: '1px solid var(--color-warning, #f59e0b)',
                }}>
                  ⚠️ No unassigned short links available.{' '}
                  <a href="/admin/shortlinks" style={{ color: 'var(--color-primary, #4f46e5)' }}>Create one on the Short Links page →</a>
                </div>
              ) : (
                <>
                  <label htmlFor="existing-link">Select existing link</label>
                  <select id="existing-link" value={formData.existingShortCode}
                    onChange={(e) => setFormData({ ...formData, existingShortCode: e.target.value })} required>
                    <option value="">Pick a short link…</option>
                    {freeShortLinks.map((l) => (
                      <option key={l.shortCode} value={l.shortCode}>/s/{l.shortCode}</option>
                    ))}
                  </select>
                  <small style={{ color: 'var(--color-muted)' }}>This link will be re-activated and pointed at the new session.</small>
                </>
              )}
            </div>
          )}

          {formData.shortlinkMode === 'custom' && (
            <div className="form-group">
              <label>Custom Short Code</label>
              <input type="text" value={formData.customShortCode}
                onChange={(e) => setFormData({ ...formData, customShortCode: e.target.value })}
                placeholder="e.g., CS101" maxLength={20}
                pattern="[a-zA-Z0-9_-]+" title="Only letters, numbers, hyphens, and underscores allowed" />
              <small style={{ color: 'var(--color-muted)' }}>Leave blank to create the session without a short link.</small>
            </div>
          )}
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
