import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import { useParams, Link } from 'react-router-dom';
import axios from 'axios';
import { toast } from 'react-toastify';
import { QrCode, CheckCircle, XCircle, X, MapPin, Clock, Calendar, RefreshCw, Users, ShieldCheck, User, AlertCircle } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import Badge from '../components/ui/Badge';
import ConfirmModal from '../components/ui/ConfirmModal';
import { SkeletonTiles, SkeletonRows } from '../components/ui/Skeleton';
import AdminSecurityReview from '../components/SecurityReview';

interface Session {
  _id: string;
  isActive: boolean;
  expiresAt: string;
  rotationCount: number;
  totpEnabled?: boolean;
  locationId?: { name: string };
  description?: string;
  batchId?: { _id: string; name: string };
  createdAt?: string;
}

interface AbsentStudent {
  name: string;
  rollNumber: string;
  collegeName: string;
}

interface AttendanceRecord {
  _id: string;
  rollNumber: string;
  studentName: string;
  photoUrl: string;
  distanceFromLocation: number;
  capturedAt: string;
  verified: boolean;
  ipAddress?: string;
  networkProvider?: string;
  userAgent?: string;
  faceDetected?: boolean;
}

interface Stats { totalAttendance: number; verifiedAttendance: number; }

type ActiveTab = 'all' | 'verified' | 'unverified' | 'absent';

const parseUA = (ua?: string) => {
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
  const { id } = useParams<{ id: string }>();

  // Core data
  const [session, setSession] = useState<Session | null>(null);
  const [attendance, setAttendance] = useState<AttendanceRecord[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);
  const [absentStudents, setAbsentStudents] = useState<AbsentStudent[]>([]);
  const [loading, setLoading] = useState(true);
  const [isExporting, setIsExporting] = useState(false);
  const [confirmAction, setConfirmAction] = useState<'rotate' | 'deactivate' | null>(null);

  // ── Verification state ──────────────────────────────────
  // Tab filter
  const [activeTab, setActiveTab] = useState<ActiveTab>('all');
  // Bulk selection
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [selectionMode, setSelectionMode] = useState<'verified' | 'unverified' | null>(null);
  // Kebab menu
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  // Single-record loading
  const [togglingId, setTogglingId] = useState<string | null>(null);
  // Undo timer ref (delayed API call)
  const undoTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ── Derived counts for tabs ─────────────────────────────
  const verifiedCount   = useMemo(() => attendance.filter(a => a.verified).length,  [attendance]);
  const unverifiedCount = useMemo(() => attendance.filter(a => !a.verified).length, [attendance]);

  // ── Filtered rows for table ─────────────────────────────
  const filteredAttendance = useMemo(() => {
    if (activeTab === 'all')        return attendance;
    if (activeTab === 'verified')   return attendance.filter(a => a.verified);
    return attendance.filter(a => !a.verified);
  }, [attendance, activeTab]);

  // ── Fetch ───────────────────────────────────────────────
  const fetchData = useCallback(async () => {
    try {
      const [sessionRes, attendanceRes, statsRes, absentRes] = await Promise.all([
        axios.get<Session>(`/api/admin/sessions/${id}`),
        axios.get<AttendanceRecord[]>(`/api/admin/sessions/${id}/attendance`),
        axios.get<Stats>(`/api/admin/sessions/${id}/stats`),
        axios.get<AbsentStudent[]>(`/api/admin/sessions/${id}/absent`).catch(() => ({ data: [] })),
      ]);
      setSession(sessionRes.data);
      setAttendance(attendanceRes.data);
      setStats(statsRes.data);
      setAbsentStudents(absentRes.data);
    } catch { toast.error('Failed to fetch session data'); }
    finally { setLoading(false); }
  }, [id]);

  useEffect(() => {
    fetchData();
    const interval = setInterval(fetchData, 30000);
    return () => clearInterval(interval);
  }, [fetchData]);

  // ── Close kebab on outside click ────────────────────────
  useEffect(() => {
    if (!openMenuId) return;
    const handler = (e: MouseEvent) => {
      if (!(e.target as Element).closest('.kebab-cell')) setOpenMenuId(null);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [openMenuId]);

  // ── Session actions ─────────────────────────────────────
  const handleRotateToken = async () => {
    try {
      const res = await axios.post<{ token: string }>(`/api/admin/sessions/${id}/rotate`);
      const { protocol, hostname } = window.location;
      const link = `${protocol}//${hostname}/attend/${res.data.token}`;
      toast.success('Token rotated! New link copied.');
      navigator.clipboard.writeText(link);
      fetchData();
    } catch { toast.error('Failed to rotate token'); }
    setConfirmAction(null);
  };

  const handleDeactivate = async () => {
    try { await axios.post(`/api/admin/sessions/${id}/deactivate`); toast.success('Session deactivated'); fetchData(); }
    catch { toast.error('Failed to deactivate session'); }
    setConfirmAction(null);
  };

  const handleExportExcel = async () => {
    if (!verifiedCount) { toast.error('No verified records to export'); return; }
    setIsExporting(true);
    try {
      const res = await axios.get(`/api/admin/sessions/${id}/export`, { responseType: 'blob' });
      const url = URL.createObjectURL(new Blob([res.data]));
      const a = document.createElement('a');
      
      const sessionDate = session?.createdAt ? new Date(session.createdAt) : new Date();
      const dateOpts: Intl.DateTimeFormatOptions = { day: '2-digit', month: 'short', year: 'numeric' };
      const dateStr = sessionDate.toLocaleDateString('en-GB', dateOpts).replace(/ /g, '_');
      
      const locName = session?.locationId?.name || 'Location';
      const batchPart = session?.batchId?.name ? `_${session.batchId.name}` : '';
      const rawName = `${locName}${batchPart}_${dateStr}`;
      const safeName = rawName.replace(/[^a-zA-Z0-9_-]/g, '_');
      
      a.href = url;
      a.download = `${safeName}.xlsx`;
      a.click();
      URL.revokeObjectURL(url);
      toast.success('Verified attendance exported!');
    } catch { toast.error('Failed to export attendance'); }
    finally { setIsExporting(false); }
  };

  // ── Tab change ──────────────────────────────────────────
  const handleTabChange = (tab: ActiveTab) => {
    if (selectedIds.size > 0) return; // lock tabs while selection is active
    setActiveTab(tab);
  };

  // ── Checkbox selection ──────────────────────────────────
  const handleRowCheck = (record: AttendanceRecord, checked: boolean) => {
    const recordStatus = record.verified ? 'verified' : 'unverified';

    // Enforce homogeneous selection
    if (checked && selectionMode && selectionMode !== recordStatus) return;

    setSelectedIds(prev => {
      const next = new Set(prev);
      if (checked) {
        next.add(record._id);
      } else {
        next.delete(record._id);
      }
      return next;
    });

    if (checked) {
      setSelectionMode(recordStatus as 'verified' | 'unverified');
    } else {
      // Release lock if nothing remains selected
      setSelectedIds(prev => {
        const next = new Set(prev);
        next.delete(record._id);
        if (next.size === 0) setSelectionMode(null);
        return next;
      });
    }
  };

  const handleSelectAll = () => {
    const eligible = filteredAttendance.filter(a =>
      !selectionMode || (selectionMode === 'verified' ? a.verified : !a.verified)
    );
    const allSelected = eligible.every(a => selectedIds.has(a._id));

    if (allSelected) {
      // Deselect all eligible
      setSelectedIds(prev => {
        const next = new Set(prev);
        eligible.forEach(a => next.delete(a._id));
        if (next.size === 0) setSelectionMode(null);
        return next;
      });
    } else {
      // Select all eligible
      const firstStatus = filteredAttendance.find(a => !selectionMode || (selectionMode === 'verified' ? a.verified : !a.verified));
      if (!firstStatus) return;
      const mode = firstStatus.verified ? 'verified' : 'unverified';
      setSelectionMode(mode);
      setSelectedIds(prev => {
        const next = new Set(prev);
        filteredAttendance
          .filter(a => (mode === 'verified' ? a.verified : !a.verified))
          .forEach(a => next.add(a._id));
        return next;
      });
    }
  };

  const clearSelection = () => {
    setSelectedIds(new Set());
    setSelectionMode(null);
  };

  // ── Single record toggle ────────────────────────────────
  const handleSingleVerify = async (record: AttendanceRecord, newVerified: boolean) => {
    setOpenMenuId(null);
    setTogglingId(record._id);
    // Optimistic update
    setAttendance(prev => prev.map(a => a._id === record._id ? { ...a, verified: newVerified } : a));
    try {
      await axios.patch(`/api/admin/attendance/${record._id}/verify`, { verified: newVerified });
      toast.success(newVerified ? `${record.studentName} marked verified` : `${record.studentName} marked unverified`);
    } catch {
      // Roll back
      setAttendance(prev => prev.map(a => a._id === record._id ? { ...a, verified: record.verified } : a));
      toast.error('Failed to update verification status');
    } finally {
      setTogglingId(null);
    }
  };

  // ── Bulk verify with undo ───────────────────────────────
  const handleBulkVerify = (newVerified: boolean) => {
    if (!selectedIds.size) return;

    const ids = Array.from(selectedIds);
    const count = ids.length;
    const label = newVerified ? 'verified' : 'unverified';

    // Snapshot for rollback
    const snapshot = [...attendance];

    // Cancel any pending undo timer
    if (undoTimerRef.current) {
      clearTimeout(undoTimerRef.current);
      toast.dismiss('bulk-verify-undo');
    }

    // Optimistic update
    setAttendance(prev => prev.map(a => selectedIds.has(a._id) ? { ...a, verified: newVerified } : a));
    clearSelection();

    // Delayed API call — 5 second undo window
    const timer = setTimeout(async () => {
      try {
        await axios.post(`/api/admin/sessions/${id}/attendance/bulk-verify`, { ids, verified: newVerified });
        toast.dismiss('bulk-verify-undo');
      } catch {
        setAttendance(snapshot);
        toast.error(`Failed to mark ${count} records as ${label}`);
        toast.dismiss('bulk-verify-undo');
      }
    }, 5000);

    undoTimerRef.current = timer;

    // Show undo toast
    toast(
      ({ closeToast }) => (
        <div className="undo-toast-body">
          <span className="undo-toast-msg">
            {count} {count === 1 ? 'record' : 'records'} marked {label}
          </span>
          <button
            className="undo-btn"
            onClick={() => {
              clearTimeout(timer);
              undoTimerRef.current = null;
              setAttendance(snapshot);
              closeToast?.();
              toast.dismiss('bulk-verify-undo');
            }}
          >
            Undo
          </button>
        </div>
      ),
      { toastId: 'bulk-verify-undo', autoClose: 5000, closeButton: false }
    );
  };

  // ── Cleanup undo timer on unmount ───────────────────────
  useEffect(() => {
    return () => {
      if (undoTimerRef.current) clearTimeout(undoTimerRef.current);
    };
  }, []);

  // ── Header checkbox state ───────────────────────────────
  const eligibleRows = useMemo(() => {
    return filteredAttendance.filter(a =>
      !selectionMode || (selectionMode === 'verified' ? a.verified : !a.verified)
    );
  }, [filteredAttendance, selectionMode]);

  const allEligibleSelected = eligibleRows.length > 0 && eligibleRows.every(a => selectedIds.has(a._id));
  const someEligibleSelected = eligibleRows.some(a => selectedIds.has(a._id)) && !allEligibleSelected;

  // ── IP duplicate detection ──────────────────────────────
  const ipCounts: Record<string, number> = {};
  attendance.forEach((a) => { if (a.ipAddress) ipCounts[a.ipAddress] = (ipCounts[a.ipAddress] || 0) + 1; });

  if (loading) return <div className="container"><SkeletonTiles count={3} /><SkeletonRows /></div>;
  if (!session) return <div className="container"><p>Session not found</p><Link to="/sessions" className="btn btn-secondary">Back to Sessions</Link></div>;

  const isExpired = new Date(session.expiresAt) < new Date();
  const statusTone = !session.isActive ? 'danger' : isExpired ? 'warning' : 'success';
  const statusLabel = !session.isActive ? 'Inactive' : isExpired ? 'Expired' : 'Active';

  return (
    <div className="container">
      <div className="row">
        <Link to="/sessions" className="btn btn-secondary btn-small">&larr; Back</Link>
      </div>

      <PageHeader title="Session Details" />

      {/* ── Session overview ───────────────────────────── */}
      <div className="session-overview-new">
        <div className="session-info-panel">
          <div className="session-info-header">
            <div className="session-info-icon-wrapper">
              <MapPin size={18} />
            </div>
            <h3 className="session-info-title">SESSION INFORMATION</h3>
          </div>

          <div className="session-details-list">
            <div className="session-detail-item">
              <span className="detail-label">
                <MapPin size={14} /> Location:
              </span>
              <span className="detail-value">{session.locationId?.name || 'Unknown'}</span>
            </div>
            <div className="session-detail-item">
              <span className="detail-label">
                <Clock size={14} /> Status:
              </span>
              <span className="detail-value">
                <span className={`status-pill status-${statusTone}`}>
                  <span className="status-dot"></span>
                  {statusLabel}
                </span>
              </span>
            </div>
            <div className="session-detail-item">
              <span className="detail-label">
                <Calendar size={14} /> Expires At:
              </span>
              <span className="detail-value">{new Date(session.expiresAt).toLocaleString()}</span>
            </div>
            <div className="session-detail-item">
              <span className="detail-label">
                <RefreshCw size={14} /> Rotations:
              </span>
              <span className="detail-value">{session.rotationCount}</span>
            </div>
            {session.batchId && (
              <div className="session-detail-item">
                <span className="detail-label">
                  <Users size={14} /> Batch:
                </span>
                <span className="detail-value">
                  <span className="status-pill" style={{ background: 'var(--color-primary-subtle)', color: 'var(--color-primary)', border: '1px solid var(--color-primary)' }}>
                    {session.batchId.name}
                  </span>
                </span>
              </div>
            )}
          </div>

          <div className="session-actions-new">
            <button
              className="btn-rotate-token"
              onClick={() => setConfirmAction('rotate')}
              disabled={!session.isActive}
            >
              <RefreshCw size={14} className="rotate-icon" /> Rotate Token
            </button>
            {session.totpEnabled && (
              <Link to={`/sessions/${id}/qr`} className="btn btn-qr-display">
                <QrCode size={14} /> View QR Display
              </Link>
            )}
            {session.isActive && !isExpired && (
              <button className="btn btn-deactivate-session" onClick={() => setConfirmAction('deactivate')}>
                Deactivate Session
              </button>
            )}
          </div>
        </div>

        <div className="session-panel-divider"></div>

        <div className="session-stats-panel">
          <div className="stat-card-new stat-card-total">
            <div className="stat-card-icon-wrapper">
              <Users size={28} />
            </div>
            <span className="stat-card-label">TOTAL ATTENDANCE</span>
            <span className="stat-card-value">{stats?.totalAttendance ?? 0}</span>
            <div className="stat-card-accent-line"></div>
          </div>

          <div className="stat-card-new stat-card-verified">
            <div className="stat-card-icon-wrapper">
              <ShieldCheck size={28} />
            </div>
            <span className="stat-card-label">VERIFIED</span>
            <span className="stat-card-value">{verifiedCount}</span>
            <div className="stat-card-accent-line"></div>
          </div>

          <div className="stat-card-new stat-card-unverified">
            <div className="stat-card-icon-wrapper" style={{ position: 'relative' }}>
              <User size={28} />
              <div style={{ position: 'absolute', bottom: 8, right: 8, background: '#ef4444', borderRadius: '50%', padding: '1px', display: 'flex', alignItems: 'center', justifyContent: 'center', border: '2px solid #180a11' }}>
                <AlertCircle size={12} color="#fff" />
              </div>
            </div>
            <span className="stat-card-label">UNVERIFIED</span>
            <span className="stat-card-value">{unverifiedCount}</span>
            <div className="stat-card-accent-line"></div>
          </div>
        </div>
      </div>

      {/* ── Security Review Section ─────────────────────── */}
      <AdminSecurityReview 
        sessionId={id || ''} 
        apiBaseUrl={import.meta.env.VITE_API_URL || '/api'}
        token={localStorage.getItem('token') || ''}
      />

      {/* ── Attendance table card ──────────────────────── */}
      <div className="card card-table">

        {/* Header row: title + export */}
        <div className="row" style={{ padding: 'var(--space-5) var(--space-6) 0' }}>
          <h3>Attendance Records</h3>
          <button
            id="export-verified-btn"
            className="btn btn-secondary btn-small"
            onClick={handleExportExcel}
            disabled={isExporting || !verifiedCount}
            title={!verifiedCount ? 'No verified records to export' : undefined}
          >
            {isExporting ? 'Exporting…' : `Export Verified${verifiedCount ? ` (${verifiedCount})` : ''}`}
          </button>
        </div>

        {/* ── Bulk action bar (appears on selection) ───── */}
        {selectedIds.size > 0 && (
          <div style={{ padding: 'var(--space-3) var(--space-6) 0' }}>
            <div className="bulk-action-bar" id="bulk-action-bar">
              <div className="bulk-info">
                <strong>{selectedIds.size}</strong>
                <span>
                  {selectedIds.size === 1 ? 'record' : 'records'} selected
                  {selectionMode && <> ({selectionMode})</>}
                </span>
              </div>
              <div className="bulk-bar-actions">
                {selectionMode === 'unverified' && (
                  <button
                    id="bulk-mark-verified-btn"
                    className="btn btn-small btn-verify"
                    onClick={() => handleBulkVerify(true)}
                  >
                    <CheckCircle size={14} />
                    Mark All Verified
                  </button>
                )}
                {selectionMode === 'verified' && (
                  <button
                    id="bulk-mark-unverified-btn"
                    className="btn btn-small btn-unverify"
                    onClick={() => handleBulkVerify(false)}
                  >
                    <XCircle size={14} />
                    Mark All Unverified
                  </button>
                )}
                <button
                  id="clear-selection-btn"
                  className="btn btn-small btn-clear-sel"
                  onClick={clearSelection}
                >
                  <X size={12} /> Clear
                </button>
              </div>
            </div>
          </div>
        )}

        {/* ── Tab filter row ────────────────────────────── */}
        <div className="row" style={{ padding: 'var(--space-4) var(--space-6) 0' }}>
          <div
            className="status-tabs"
            title={selectedIds.size > 0 ? 'Clear selection before switching tabs' : undefined}
          >
            {(['all', 'verified', 'unverified', ...(session.batchId ? ['absent'] : [])] as ActiveTab[]).map(tab => (
              <button
                key={tab}
                id={`tab-${tab}`}
                className={`status-tab tab-${tab}${activeTab === tab ? ' active' : ''}`}
                onClick={() => handleTabChange(tab)}
                disabled={selectedIds.size > 0}
                style={selectedIds.size > 0 ? { cursor: 'not-allowed', opacity: .5 } : undefined}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
                <span className="tab-count">
                  {tab === 'all' ? attendance.length : tab === 'verified' ? verifiedCount : tab === 'unverified' ? unverifiedCount : absentStudents.length}
                </span>
              </button>
            ))}
          </div>
        </div>

        {/* ── Table ─────────────────────────────────────── */}
        {activeTab === 'absent' ? (
          absentStudents.length === 0 ? (
            <div className="empty-state" style={{ marginTop: 'var(--space-4)', padding: '3rem' }}>
              <CheckCircle size={48} className="text-success" style={{ marginBottom: '1rem', opacity: 0.8 }} />
              <h3>Perfect Attendance!</h3>
              <p>All students in this batch have successfully checked in.</p>
            </div>
          ) : (
            <div className="table-scroll" style={{ marginTop: 'var(--space-4)' }}>
              <table className="table" style={{ minWidth: 600 }}>
                <thead>
                  <tr>
                    <th style={{ textAlign: 'left' }}>Roll No.</th>
                    <th style={{ textAlign: 'left' }}>Student Name</th>
                    <th style={{ textAlign: 'left' }}>College Name</th>
                    <th style={{ textAlign: 'center' }}>Status</th>
                  </tr>
                </thead>
                <tbody>
                  {absentStudents.map(student => (
                    <tr key={student.rollNumber}>
                      <td><strong>{student.rollNumber}</strong></td>
                      <td>{student.name}</td>
                      <td>{student.collegeName || '-'}</td>
                      <td style={{ textAlign: 'center' }}>
                        <Badge tone="danger">Absent</Badge>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )
        ) : filteredAttendance.length === 0 ? (
          <p style={{ padding: 'var(--space-6)' }}>
            {attendance.length === 0 ? 'No attendance records yet' : `No ${activeTab} records`}
          </p>
        ) : (
          <div className="table-scroll" style={{ marginTop: 'var(--space-4)' }}>
            <table className="table" style={{ minWidth: 720 }}>
              <colgroup>
                <col style={{ width: '44px' }} />
                <col style={{ width: '80px' }} />
                <col style={{ width: '170px' }} />
                <col style={{ width: '60px' }} />
                <col style={{ width: '80px' }} />
                <col style={{ width: '90px' }} />
                <col style={{ width: '100px' }} />
                <col style={{ width: '150px' }} />
                <col style={{ width: '130px' }} />
                <col style={{ width: '100px' }} />
                <col style={{ width: '36px' }} />
              </colgroup>
              <thead>
                <tr>
                  {/* Select-all checkbox */}
                  <th className="col-check">
                    <input
                      type="checkbox"
                      id="select-all-checkbox"
                      className="header-checkbox"
                      checked={allEligibleSelected}
                      ref={el => { if (el) el.indeterminate = someEligibleSelected; }}
                      onChange={handleSelectAll}
                    />
                  </th>
                  <th style={{ textAlign: 'left' }}>Roll No.</th>
                  <th style={{ textAlign: 'left' }}>Name</th>
                  <th style={{ textAlign: 'center' }}>Photo</th>
                  <th style={{ textAlign: 'left' }}>Distance</th>
                  <th style={{ textAlign: 'left' }}>Time</th>
                  <th style={{ textAlign: 'center' }}>Status</th>
                  <th style={{ textAlign: 'left' }}>IP Address</th>
                  <th style={{ textAlign: 'left' }}>Device</th>
                  <th style={{ textAlign: 'center' }}>Face Check</th>
                  <th className="kebab-cell" />
                </tr>
              </thead>
              <tbody>
                {filteredAttendance.map(record => {
                  const isSelected = selectedIds.has(record._id);
                  const recordStatus = record.verified ? 'verified' : 'unverified';
                  const isDisabled = !!(selectionMode && selectionMode !== recordStatus);
                  const isToggling = togglingId === record._id;
                  const isMenuOpen = openMenuId === record._id;

                  return (
                    <tr
                      key={record._id}
                      style={isSelected ? { background: 'var(--color-primary-light)' } : undefined}
                    >
                      {/* Checkbox */}
                      <td className="col-check">
                        <div
                          className="checkbox-wrapper"
                          data-disabled={isDisabled ? 'true' : undefined}
                          data-tip="Select same-status records only"
                        >
                          <input
                            type="checkbox"
                            id={`check-${record._id}`}
                            className="row-checkbox"
                            checked={isSelected}
                            disabled={isDisabled}
                            onChange={e => handleRowCheck(record, e.target.checked)}
                          />
                        </div>
                      </td>

                      <td style={{ fontWeight: 500 }}>{record.rollNumber}</td>

                      <td>
                        <span title={record.studentName} style={{ display: 'block', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                          {record.studentName}
                        </span>
                      </td>

                      <td style={{ textAlign: 'center' }}>
                        <img src={record.photoUrl} alt="Student" style={{ width: '44px', height: '44px', objectFit: 'cover', borderRadius: 'var(--radius-sm)' }} />
                      </td>

                      <td><span style={{ whiteSpace: 'nowrap' }}>{record.distanceFromLocation}m</span></td>

                      <td><span style={{ whiteSpace: 'nowrap' }}>{new Date(record.capturedAt).toLocaleTimeString()}</span></td>

                      <td style={{ textAlign: 'center' }}>
                        <Badge tone={record.verified ? 'success' : 'danger'}>
                          {record.verified ? 'Verified' : 'Unverified'}
                        </Badge>
                      </td>

                      <td>
                        <div>
                          <div style={{ fontWeight: 500, whiteSpace: 'nowrap' }}>{record.ipAddress || 'N/A'}</div>
                          {record.networkProvider && <div style={{ fontSize: '11px', color: 'var(--color-muted)', marginTop: '2px', whiteSpace: 'nowrap' }}>{record.networkProvider}</div>}
                          {record.ipAddress && ipCounts[record.ipAddress] > 1 && <Badge tone="danger">Shared ({ipCounts[record.ipAddress]})</Badge>}
                        </div>
                      </td>

                      <td><span title={record.userAgent}>{parseUA(record.userAgent)}</span></td>

                      <td style={{ textAlign: 'center' }}>
                        <Badge tone={record.faceDetected !== false ? 'success' : 'danger'}>
                          {record.faceDetected !== false ? 'Detected' : 'No Face'}
                        </Badge>
                      </td>

                      {/* ⋮ Kebab menu */}
                      <td className="kebab-cell">
                        <button
                          id={`kebab-btn-${record._id}`}
                          className={`kebab-btn${isMenuOpen ? ' open' : ''}`}
                          aria-label="Actions"
                          disabled={isToggling}
                          onClick={() => setOpenMenuId(isMenuOpen ? null : record._id)}
                        >
                          ⋮
                        </button>

                        {isMenuOpen && (
                          <div className="kebab-dropdown" role="menu">
                            {!record.verified ? (
                              <button
                                id={`mark-verified-${record._id}`}
                                className="kebab-item verify-action"
                                role="menuitem"
                                onClick={() => handleSingleVerify(record, true)}
                              >
                                <CheckCircle size={14} />
                                Mark as Verified
                              </button>
                            ) : (
                              <button
                                id={`mark-unverified-${record._id}`}
                                className="kebab-item unverify-action"
                                role="menuitem"
                                onClick={() => handleSingleVerify(record, false)}
                              >
                                <XCircle size={14} />
                                Mark as Unverified
                              </button>
                            )}
                          </div>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      <ConfirmModal
        isOpen={confirmAction !== null}
        title={confirmAction === 'rotate' ? 'Rotate Token' : 'Deactivate Session'}
        message={confirmAction === 'rotate' ? 'Rotate token? The current link will stop working.' : 'Are you sure you want to deactivate this session? Students will no longer be able to submit attendance.'}
        confirmText={confirmAction === 'rotate' ? 'Rotate' : 'Deactivate'}
        onConfirm={confirmAction === 'rotate' ? handleRotateToken : handleDeactivate}
        onCancel={() => setConfirmAction(null)}
      />
    </div>
  );
};

export default SessionDetail;
