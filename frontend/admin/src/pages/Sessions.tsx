import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { Link } from 'react-router';
import { toast } from 'react-toastify';
import { ClipboardList, Sparkles, Link as LinkIcon, Pencil, AlertTriangle, FileSpreadsheet, Download } from 'lucide-react';
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
import SessionFilters from '../components/ui/SessionFilters';
import MultiSelect from '../components/ui/MultiSelect';
import ExcelSessionUploadModal from '../components/sessions/ExcelSessionUploadModal';
import { downloadBlob, filenameFromContentDisposition } from '../utils/downloadBlob';

interface Location { _id: string; name: string; radiusMeters: number; }
interface ShortLink { _id: string; shortCode: string; isActive: boolean; sessionId?: unknown; }
interface Batch { _id: string; name: string; studentCount: number; type?: 'manual' | 'session'; }
export interface Mentor { _id: string; username: string; fullName?: string; role: string; isActive: boolean; email?: string; }
type ShortlinkMode = 'auto' | 'custom' | 'existing' | 'none';
interface Session {
  _id: string;
  locationId?: Location | string;
  locationName?: string;
  isActive: boolean;
  expiresAt: string;
  createdAt: string;
  attendanceCount: number;
  description?: string;
  assignedAdminNames?: string[];
  collegeName?: string;
  startsAt?: string;
}

const getLocationName = (s: Session) => {
  if (s.locationName) return s.locationName;
  if (typeof s.locationId === 'object' && s.locationId?.name) return s.locationId.name;
  return 'Unknown';
};

const getInitialFilters = () => {
  try {
    const saved = sessionStorage.getItem('sessionFilters');
    return saved ? JSON.parse(saved) : { locationId: '', date: '' };
  } catch {
    return { locationId: '', date: '' };
  }
};


const Sessions = () => {
  const [sessions, setSessions] = useState<Session[]>([]);
  const [locations, setLocations] = useState<Location[]>([]);
  const [batches, setBatches] = useState<Batch[]>([]);
  const [mentors, setMentors] = useState<Mentor[]>([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const emptyFormData = {
    sessionType: 'normal' as 'normal' | 'exam',
    locationId: '',
    durationMinutes: 30,
    description: '',
    shortlinkMode: 'auto' as ShortlinkMode,
    customShortCode: '',
    existingShortCode: '',
    batchId: '',
    assignedAdminIds: [] as string[],
    collegeName: '',
    startsDate: '',
    startsTime: '',
  };
  const [formData, setFormData] = useState(emptyFormData);
  const [activeShortLinks, setActiveShortLinks] = useState<ShortLink[]>([]);
  const [reassignConfirm, setReassignConfirm] = useState({ open: false, shortCode: '' });
  const [deleteModal, setDeleteModal] = useState({ open: false, sessionId: '', attendanceCount: 0, locationName: '' });
  const [deletePassword, setDeletePassword] = useState('');
  const [deleting, setDeleting] = useState(false);
  const [deactivateId, setDeactivateId] = useState<string | null>(null);
  const [showUploadModal, setShowUploadModal] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [bulkExporting, setBulkExporting] = useState(false);
  const [bulkDeleteModal, setBulkDeleteModal] = useState(false);
  const [bulkDeletePassword, setBulkDeletePassword] = useState('');
  const [bulkDeleting, setBulkDeleting] = useState(false);
  const abortRef = useRef<AbortController | null>(null);
  const filtersRef = useRef<{ locationId: string; date: string }>(getInitialFilters());

  const fetchData = useCallback(async () => {
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    try {
      const filters = filtersRef.current;
      const params: Record<string, string> = {};
      if (filters.locationId) params.locationId = filters.locationId;
      if (filters.date) params.date = filters.date;

      const [sessionsRes, locationsRes, shortLinksRes, batchesRes, mentorsRes] = await Promise.all([
        axios.get<Session[]>('/api/admin/sessions', { params, signal: abortRef.current.signal }),
        axios.get<Location[]>('/api/admin/locations', { signal: abortRef.current.signal }),
        axios.get<{ shortLinks: ShortLink[] }>('/api/admin/shortlinks', { signal: abortRef.current.signal }),
        axios.get<Batch[]>('/api/admin/batches', { signal: abortRef.current.signal }),
        axios.get<Mentor[]>('/api/admin/users', { signal: abortRef.current.signal }),
      ]);
      setSessions(sessionsRes.data);
      setLocations(locationsRes.data);
      setActiveShortLinks((shortLinksRes.data.shortLinks ?? []).filter((l) => l.isActive || !l.sessionId));
      // Session-batches (Excel-upload bulk creation) are 1:1 with the
      // session that generated them — they don't belong in the manual
      // "pick a batch" dropdown below.
      setBatches(batchesRes.data.filter((b) => b.type !== 'session'));
      setMentors(mentorsRes.data.filter((m) => m.role === 'admin' && m.isActive));
    } catch (error) {
      if ((error as { name?: string }).name !== 'CanceledError') toast.error('Failed to fetch data');
    } finally { setLoading(false); }
  }, []);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 60000);
    return () => { clearInterval(interval); abortRef.current?.abort(); };
  }, [fetchData]);

  const handleFilterChange = useCallback((newFilters: { locationId: string; date: string }) => {
    filtersRef.current = newFilters;
    fetchData();
  }, [fetchData]);

  const isExpired = (session: Session) => new Date(session.expiresAt) < new Date();

  const getStatus = (session: Session) => {
    if (!session.isActive) return { label: 'Inactive', tone: 'danger' as const };
    if (isExpired(session)) return { label: 'Expired', tone: 'warning' as const };
    return { label: 'Active', tone: 'success' as const };
  };

  const handleCreateSubmit = async (e?: FormEvent, forceReassign = false) => {
    if (e) e.preventDefault();
    try {
      const duration = parseInt(String(formData.durationMinutes));
      if (isNaN(duration) || duration < 5 || duration > 480) { toast.error('Duration must be between 5 and 480 minutes'); return; }
      const isExam = formData.sessionType === 'exam';
      if (isExam) {
        if (!formData.batchId) { toast.error('A batch is required for an exam session'); return; }
        if (formData.assignedAdminIds.length === 0) { toast.error('Assign at least one mentor to this exam session'); return; }
        if (!formData.collegeName.trim()) { toast.error('College name is required for an exam session'); return; }
        if (!formData.startsDate || !formData.startsTime) { toast.error('Start date and time are required for an exam session'); return; }
      } else if (!formData.locationId) {
        toast.error('A location is required for a self check-in session');
        return;
      }
      if (formData.shortlinkMode === 'existing' && !formData.existingShortCode) {
        toast.error('Please select an existing short link, or switch to a different mode.');
        return;
      }

      // Check for reassigning existing short link confirmation
      if (formData.shortlinkMode === 'existing' && formData.existingShortCode && !forceReassign) {
        const selectedLink = activeShortLinks.find(l => l.shortCode === formData.existingShortCode);
        if (selectedLink && selectedLink.sessionId) {
          setReassignConfirm({ open: true, shortCode: formData.existingShortCode });
          return;
        }
      }

      const res = await axios.post<{ _id: string; shortCode?: string }>('/api/admin/sessions', {
        // Exam sessions have no location — manual attendance, not geofenced.
        locationId: isExam ? undefined : formData.locationId,
        durationMinutes: duration,
        description: formData.description,
        // Normal sessions omit these entirely — an empty mentor list is
        // exactly what tells the backend "this is a normal session, batch
        // stays optional" (see SessionCreateRequest::is_exam_session).
        batchId: isExam ? formData.batchId : (formData.batchId || undefined),
        assignedAdminIds: isExam ? formData.assignedAdminIds : undefined,
        collegeName: isExam ? formData.collegeName.trim() : undefined,
        startsAt: isExam ? new Date(`${formData.startsDate}T${formData.startsTime}`).toISOString() : undefined,
        // Exam sessions have no self-service check-in flow, so there's
        // nothing for a short link to point to.
        shortlinkMode: isExam ? 'none' : formData.shortlinkMode,
        customShortCode: isExam ? undefined : (formData.customShortCode.trim() || undefined),
        existingShortCode: isExam ? undefined : (formData.existingShortCode || undefined),
      });

      const { protocol, hostname } = window.location;
      let successMessage = 'Session created successfully!';

      if (res.data.shortCode) {
        const link = `${protocol}//${hostname}/s/${res.data.shortCode}`;
        await navigator.clipboard.writeText(link).catch(() => {});
        successMessage = `Session created! Link (/s/${res.data.shortCode}) copied to clipboard.`;
      }

      toast.success(successMessage);
      setShowModal(false);
      setFormData(emptyFormData);
      fetchData();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string; error?: string } } };
      toast.error(err.response?.data?.message || err.response?.data?.error || 'Failed to create session');
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

  const toggleSelect = (id: string, checked: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (checked) next.add(id); else next.delete(id);
      return next;
    });
  };

  const allSelected = sortedSessions.length > 0 && sortedSessions.every((s) => selectedIds.has(s._id));
  const someSelected = sortedSessions.some((s) => selectedIds.has(s._id)) && !allSelected;

  const toggleSelectAll = () => {
    if (allSelected) { setSelectedIds(new Set()); return; }
    setSelectedIds(new Set(sortedSessions.map((s) => s._id)));
  };

  const handleBulkExport = async () => {
    if (selectedIds.size === 0) return;
    setBulkExporting(true);
    try {
      const res = await axios.post('/api/admin/sessions/export-bulk', { sessionIds: [...selectedIds] }, { responseType: 'blob' });
      const filename = filenameFromContentDisposition(res.headers['content-disposition'], `Attendance_Export_${selectedIds.size}sessions.xlsx`);
      downloadBlob(new Blob([res.data]), filename);
      toast.success(`Exported ${selectedIds.size} session${selectedIds.size === 1 ? '' : 's'}`);
      setSelectedIds(new Set());
    } catch {
      toast.error('Failed to export selected sessions');
    } finally {
      setBulkExporting(false);
    }
  };

  const handleBulkDelete = async (e: FormEvent) => {
    e.preventDefault();
    if (!bulkDeletePassword) { toast.error('Please enter your admin password'); return; }
    setBulkDeleting(true);
    try {
      const res = await axios.post('/api/admin/sessions/delete-bulk', {
        sessionIds: [...selectedIds],
        password: bulkDeletePassword,
      });
      const { deletedCount, failedIds } = res.data as { deletedCount: number; failedIds: string[] };
      if (failedIds && failedIds.length > 0) {
        toast.error(`Deleted ${deletedCount} session${deletedCount === 1 ? '' : 's'}; ${failedIds.length} could not be deleted`);
      } else {
        toast.success(`Deleted ${deletedCount} session${deletedCount === 1 ? '' : 's'}`);
      }
      setBulkDeleteModal(false);
      setBulkDeletePassword('');
      setSelectedIds(new Set());
      fetchData();
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to delete selected sessions');
    } finally {
      setBulkDeleting(false);
    }
  };

  const columns: Column<Session>[] = [
    { key: 'select', width: '4%', label: (
      <input
        type="checkbox"
        checked={allSelected}
        ref={(el) => { if (el) el.indeterminate = someSelected; }}
        onChange={toggleSelectAll}
        aria-label="Select all sessions"
      />
    ), render: (s) => (
      <input
        type="checkbox"
        checked={selectedIds.has(s._id)}
        onChange={(e) => toggleSelect(s._id, e.target.checked)}
        aria-label={`Select session ${s._id}`}
      />
    )},
    { key: 'location', label: 'Location',   width: '14%', render: (s) => getLocationName(s) },
    { key: 'college',  label: 'College',    width: '14%', render: (s) => s.collegeName || <span style={{ color: 'var(--color-muted)' }}>—</span> },
    { key: 'mentor',   label: 'Mentor',     width: '13%', render: (s) => (s.assignedAdminNames && s.assignedAdminNames.length > 0) ? s.assignedAdminNames.join(', ') : <span style={{ color: 'var(--color-muted)' }}>—</span> },
    { key: 'status',   label: 'Status',     width: '10%', render: (s) => { const st = getStatus(s); return <Badge tone={st.tone}>{st.label}</Badge>; }},
    { key: 'starts',   label: 'Starts At',  width: '14%', render: (s) => s.startsAt ? new Date(s.startsAt).toLocaleString() : '—' },
    { key: 'students', label: 'Students',   width: '9%', align: 'center', render: (s) => s.attendanceCount },
    { key: 'actions',  label: 'Actions',    width: '25%', render: (s) => (
      <div className="actions-cell">
        <Link to={`/sessions/${s._id}`} className="btn btn-secondary btn-small">View</Link>
        {s.isActive && !isExpired(s) && <Button variant="danger" size="sm" onClick={() => setDeactivateId(s._id)}>Deactivate</Button>}
        <Button variant="delete" size="sm" onClick={() => { setDeleteModal({ open: true, sessionId: s._id, attendanceCount: s.attendanceCount, locationName: getLocationName(s) }); setDeletePassword(''); }}>Delete</Button>
      </div>
    )},
  ];

  return (
    <div className="container">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4 mb-4">
        <PageHeader title="Attendance Sessions">
          {/* Not gated on locations existing — exam sessions don't need one. */}
          <button className="btn btn-primary" onClick={() => setShowModal(true)}>Create Session</button>
          <button className="btn btn-secondary" onClick={() => setShowUploadModal(true)}>
            <FileSpreadsheet size={15} style={{ marginRight: 6, verticalAlign: -2 }} />
            Upload Excel
          </button>
        </PageHeader>
        <SessionFilters locations={locations} onFilterChange={handleFilterChange} />
      </div>

      {locations.length === 0 && (
        <div className="card"><p>No locations found. <Link to="/locations">Create one</Link> if you need a self check-in (normal) session — exam sessions don't require a location.</p></div>
      )}

      {selectedIds.size > 0 && (
        <div className="row mb-4" style={{ justifyContent: 'flex-start', gap: 12 }}>
          <span style={{ fontSize: '0.85rem', color: 'var(--color-muted)' }}>
            <strong>{selectedIds.size}</strong> session{selectedIds.size === 1 ? '' : 's'} selected
          </span>
          <Button variant="secondary" size="sm" onClick={handleBulkExport} disabled={bulkExporting}>
            <Download size={14} style={{ marginRight: 6, verticalAlign: -2 }} />
            {bulkExporting ? 'Exporting…' : `Export Selected (${selectedIds.size})`}
          </Button>
          <Button variant="delete" size="sm" onClick={() => { setBulkDeleteModal(true); setBulkDeletePassword(''); }}>
            {`Delete Selected (${selectedIds.size})`}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => setSelectedIds(new Set())}>Clear</Button>
        </div>
      )}

      {loading ? <SkeletonRows /> : sessions.length === 0 ? (
        <EmptyState icon={ClipboardList} title="No sessions yet" message="Create your first attendance session!" />
      ) : sessions.length > 0 ? (
        <div className="card card-table">
          <DataTable columns={columns} rows={sortedSessions} rowKey={(s) => s._id} />
        </div>
      ) : null}

      <ExcelSessionUploadModal
        open={showUploadModal}
        onClose={() => setShowUploadModal(false)}
        mentors={mentors}
        onMentorCreated={(m) => setMentors((prev) => [...prev, m])}
        onSessionsCreated={fetchData}
      />

      <Modal open={showModal} onClose={() => setShowModal(false)} title="Create Attendance Session">
        <form onSubmit={handleCreateSubmit}>
          <div className="form-group">
            <label>Session Type</label>
            <div style={{ display: 'flex', gap: '0.4rem', marginTop: '0.375rem' }}>
              {(['normal', 'exam'] as const).map((type) => (
                <label key={type} style={{
                  flex: 1, textAlign: 'center', padding: '0.45rem 0.25rem',
                  borderRadius: '6px', cursor: 'pointer', fontSize: '0.78rem', fontWeight: 500,
                  border: `1.5px solid ${
                    formData.sessionType === type ? 'var(--color-primary, #4f46e5)' : 'var(--color-border, #e5e7eb)'
                  }`,
                  background: formData.sessionType === type ? 'var(--color-primary-subtle, #eef2ff)' : 'transparent',
                  color: formData.sessionType === type ? 'var(--color-primary, #4f46e5)' : 'var(--color-muted)',
                  transition: 'all 0.15s',
                  userSelect: 'none',
                }}>
                  <input type="radio" name="sessionType" value={type}
                    checked={formData.sessionType === type}
                    onChange={() => setFormData({ ...formData, sessionType: type })}
                    style={{ display: 'none' }} />
                  {type === 'normal' ? 'Normal (self check-in)' : 'Exam (assign a mentor)'}
                </label>
              ))}
            </div>
            {formData.sessionType === 'exam' && (
              <small style={{ color: 'var(--color-muted)' }}>Exam sessions require a batch, one or more mentors, a college name, and a start date/time. Attendance is marked manually, so no location is needed.</small>
            )}
          </div>

          {formData.sessionType === 'normal' && (
            <div className="form-group">
              <label htmlFor="session-location">Location</label>
              <select id="session-location" value={formData.locationId} onChange={(e) => setFormData({ ...formData, locationId: e.target.value })} required>
                <option value="">Select a location</option>
                {locations.map((loc) => <option key={loc._id} value={loc._id}>{loc.name} (Radius: {loc.radiusMeters}m)</option>)}
              </select>
            </div>
          )}
          <div className="form-group">
            <label>Duration (minutes)</label>
            <input type="number" value={formData.durationMinutes} onChange={(e) => setFormData({ ...formData, durationMinutes: parseInt(e.target.value) })} min="5" max="480" required />
          </div>
          <div className="form-group">
            <label>Description (optional)</label>
            <textarea value={formData.description} onChange={(e) => setFormData({ ...formData, description: e.target.value })} rows={2} placeholder="e.g., Morning attendance for CS101" />
          </div>

          <div className="form-group">
            <label htmlFor="session-batch">Batch{formData.sessionType === 'normal' ? ' (Optional)' : ''}</label>
            <select id="session-batch" value={formData.batchId} onChange={(e) => setFormData({ ...formData, batchId: e.target.value })} required={formData.sessionType === 'exam'}>
              <option value="">{formData.sessionType === 'exam' ? 'Select a batch' : 'No Batch'}</option>
              {(batches || []).map((batch) => (
                <option key={batch._id} value={batch._id}>{batch.name} ({batch.studentCount} students)</option>
              ))}
            </select>
            {formData.sessionType === 'exam' && batches.length === 0 && (
              <small style={{ color: 'var(--color-danger)' }}>No batches found. <Link to="/batches">Create one first</Link>.</small>
            )}
          </div>

          {formData.sessionType === 'exam' && (
            <>
              <div className="form-group">
                <label htmlFor="session-mentor-list">Assign Mentor(s)</label>
                <MultiSelect
                  id="session-mentor-list"
                  options={mentors.map((m) => ({ value: m._id, label: m.fullName || m.username }))}
                  selected={formData.assignedAdminIds}
                  onChange={(next) => setFormData({ ...formData, assignedAdminIds: next })}
                  placeholder={mentors.length === 0 ? 'No active mentor accounts' : 'Search mentors…'}
                  emptyMessage="No mentors match your search"
                />
                {mentors.length === 0 && (
                  <small style={{ color: 'var(--color-danger)' }}>No active mentor accounts. <Link to="/users">Create one first</Link>.</small>
                )}
              </div>

              <div className="form-group">
                <label htmlFor="session-college">College Name</label>
                <input id="session-college" type="text" value={formData.collegeName} onChange={(e) => setFormData({ ...formData, collegeName: e.target.value })} placeholder="e.g., XYZ Engineering College" required />
              </div>

              <div className="form-row">
                <div className="form-group">
                  <label htmlFor="session-starts-date">Exam Date</label>
                  <input id="session-starts-date" type="date" value={formData.startsDate} onChange={(e) => setFormData({ ...formData, startsDate: e.target.value })} required />
                </div>
                <div className="form-group">
                  <label htmlFor="session-starts-time">Start Time</label>
                  <input id="session-starts-time" type="time" value={formData.startsTime} onChange={(e) => setFormData({ ...formData, startsTime: e.target.value })} required />
                </div>
              </div>
            </>
          )}

          {/* ── Short Link mode selector — exam sessions have no self-service
               check-in flow to link to, so this is normal-session only ── */}
          {formData.sessionType === 'normal' && (
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
          )}

          {formData.sessionType === 'normal' && formData.shortlinkMode === 'auto' && (
            <p style={{ fontSize: '0.8rem', color: 'var(--color-muted)', marginTop: '-0.25rem', marginBottom: '0.5rem' }}>
              A random 6-character link will be created and copied to your clipboard.
            </p>
          )}

          {formData.sessionType === 'normal' && formData.shortlinkMode === 'existing' && (
            <div className="form-group">
              {activeShortLinks.length === 0 ? (
                <div style={{
                  padding: '0.75rem', borderRadius: '6px', fontSize: '0.82rem',
                  background: 'var(--color-warning-subtle, #fffbeb)',
                  border: '1px solid var(--color-warning, #f59e0b)',
                }}>
                  ⚠️ No active short links available.{' '}
                  <Link to="/shortlinks" style={{ color: 'var(--color-primary, #4f46e5)' }}>Create one on the Short Links page →</Link>
                </div>
              ) : (
                <>
                  <label htmlFor="existing-link">Select existing link</label>
                  <select id="existing-link" value={formData.existingShortCode}
                    onChange={(e) => setFormData({ ...formData, existingShortCode: e.target.value })} required>
                    <option value="">Pick a short link…</option>
                    {activeShortLinks.map((l) => (
                      <option key={l.shortCode} value={l.shortCode}>
                        /s/{l.shortCode} {l.sessionId ? '(In use)' : '(Available)'}
                      </option>
                    ))}
                  </select>
                  <small style={{ color: 'var(--color-muted)' }}>If the link is in use, you will be asked to confirm reassignment.</small>
                </>
              )}
            </div>
          )}

          {formData.sessionType === 'normal' && formData.shortlinkMode === 'custom' && (
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

      <ConfirmDialog
        open={bulkDeleteModal}
        onClose={() => { setBulkDeleteModal(false); setBulkDeletePassword(''); }}
        onSubmit={handleBulkDelete}
        title="Delete Selected Sessions"
        confirmLabel="Confirm Delete"
        loading={bulkDeleting}
        message={
          <span style={{ color: 'var(--color-danger)' }}>
            You are about to permanently delete <strong>{selectedIds.size}</strong> session{selectedIds.size === 1 ? '' : 's'} and all of their attendance records and photos. This cannot be undone.
          </span>
        }
      >
        <div className="form-group">
          <label>Confirm with Admin Password</label>
          <input type="password" value={bulkDeletePassword} onChange={(e) => setBulkDeletePassword(e.target.value)} placeholder="Enter your admin password" autoFocus required />
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

      <Modal open={reassignConfirm.open} onClose={() => setReassignConfirm({ open: false, shortCode: '' })} title="">
        <div style={{ textAlign: 'center', padding: '1rem 0.5rem' }}>
          <div style={{ 
            width: '64px', height: '64px', borderRadius: '16px', 
            background: 'linear-gradient(135deg, rgba(245, 158, 11, 0.2), rgba(217, 119, 6, 0.2))',
            border: '1px solid rgba(245, 158, 11, 0.3)',
            display: 'flex', alignItems: 'center', justifyContent: 'center', margin: '0 auto 1.5rem',
            boxShadow: '0 8px 32px rgba(245, 158, 11, 0.15)'
          }}>
            <AlertTriangle size={32} color="#fbbf24" />
          </div>
          <h3 style={{ fontSize: '1.25rem', fontWeight: 700, marginBottom: '1rem', color: '#fff' }}>Short Link In Use</h3>
          <p style={{ color: 'var(--color-muted)', lineHeight: 1.6, marginBottom: '2rem' }}>
            The short link <strong style={{ color: '#fff' }}>/s/{reassignConfirm.shortCode}</strong> is currently attached to another active session. 
            <br/><br/>
            If you continue, it will be forcefully reassigned to this new session, and the previous session will lose this link.
          </p>
          <div style={{ display: 'flex', gap: '1rem', justifyContent: 'center' }}>
            <button type="button" className="btn btn-secondary" onClick={() => setReassignConfirm({ open: false, shortCode: '' })}>
              Cancel
            </button>
            <button type="button" 
              onClick={() => { setReassignConfirm({ open: false, shortCode: '' }); handleCreateSubmit(undefined, true); }}
              style={{
                background: 'linear-gradient(135deg, #f59e0b, #d97706)',
                color: '#fff', border: 'none', padding: '10px 24px', borderRadius: 'var(--radius-sm)',
                fontWeight: 600, cursor: 'pointer', boxShadow: '0 4px 12px rgba(245, 158, 11, 0.25)',
                transition: 'transform 0.2s ease, box-shadow 0.2s ease'
              }}
              onMouseOver={(e) => e.currentTarget.style.transform = 'translateY(-1px)'}
              onMouseOut={(e) => e.currentTarget.style.transform = 'translateY(0)'}
            >
              Reassign & Create
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
};

export default Sessions;
