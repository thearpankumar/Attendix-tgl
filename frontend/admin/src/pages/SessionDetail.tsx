import { useEffect, useState, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import axios from 'axios';
import { toast } from 'react-toastify';
import { QrCode } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import StatTile from '../components/ui/StatTile';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import Badge from '../components/ui/Badge';
import ConfirmModal from '../components/ui/ConfirmModal';
import { SkeletonTiles, SkeletonRows } from '../components/ui/Skeleton';

interface Session {
  _id: string;
  isActive: boolean;
  expiresAt: string;
  rotationCount: number;
  totpEnabled?: boolean;
  locationId?: { name: string };
  description?: string;
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
  const [session, setSession] = useState<Session | null>(null);
  const [attendance, setAttendance] = useState<AttendanceRecord[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);
  const [loading, setLoading] = useState(true);
  const [confirmAction, setConfirmAction] = useState<'rotate' | 'deactivate' | null>(null);

  const fetchData = useCallback(async () => {
    try {
      const [sessionRes, attendanceRes, statsRes] = await Promise.all([
        axios.get<Session>(`/api/admin/sessions/${id}`),
        axios.get<AttendanceRecord[]>(`/api/admin/sessions/${id}/attendance`),
        axios.get<Stats>(`/api/admin/sessions/${id}/stats`),
      ]);
      setSession(sessionRes.data);
      setAttendance(attendanceRes.data);
      setStats(statsRes.data);
    } catch { toast.error('Failed to fetch session data'); }
    finally { setLoading(false); }
  }, [id]);

  useEffect(() => { fetchData(); const interval = setInterval(fetchData, 30000); return () => clearInterval(interval); }, [fetchData]);

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

  const handleExportCSV = () => {
    if (!attendance.length) { toast.error('No attendance data to export'); return; }
    const headers = ['Roll Number', 'Name', 'Verified', 'Distance (m)', 'IP Address', 'Network Provider', 'Device', 'Face Detected', 'Captured At'];
    const csv = [
      headers.join(','),
      ...attendance.map((a) => [
        a.rollNumber, `"${a.studentName}"`, a.verified ? 'Yes' : 'No',
        a.distanceFromLocation, a.ipAddress || 'N/A', `"${a.networkProvider || 'N/A'}"`,
        `"${parseUA(a.userAgent)}"`, a.faceDetected !== false ? 'Yes' : 'No',
        new Date(a.capturedAt).toISOString(),
      ].join(',')),
    ].join('\n');
    const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv' }));
    const a = document.createElement('a');
    a.href = url; a.download = `attendance-${id}-${new Date().toISOString().split('T')[0]}.csv`;
    a.click(); URL.revokeObjectURL(url);
  };

  if (loading) return <div className="container"><SkeletonTiles count={3} /><SkeletonRows /></div>;
  if (!session) return <div className="container"><p>Session not found</p><Link to="/sessions" className="btn btn-secondary">Back to Sessions</Link></div>;

  const isExpired = new Date(session.expiresAt) < new Date();
  const statusTone = !session.isActive ? 'danger' : isExpired ? 'warning' : 'success';
  const statusLabel = !session.isActive ? 'Inactive' : isExpired ? 'Expired' : 'Active';

  const ipCounts: Record<string, number> = {};
  attendance.forEach((a) => { if (a.ipAddress) ipCounts[a.ipAddress] = (ipCounts[a.ipAddress] || 0) + 1; });

  const columns: Column<AttendanceRecord>[] = [
    { key: 'rollNumber', label: 'Roll No.', render: (a) => a.rollNumber },
    { key: 'name',       label: 'Name',     render: (a) => a.studentName },
    { key: 'photo',      label: 'Photo',    render: (a) => <img src={a.photoUrl} alt="Student" style={{ width: '44px', height: '44px', objectFit: 'cover', borderRadius: 'var(--radius-sm)' }} /> },
    { key: 'distance',   label: 'Distance', render: (a) => `${a.distanceFromLocation}m` },
    { key: 'time',       label: 'Time',     render: (a) => new Date(a.capturedAt).toLocaleTimeString() },
    { key: 'status',     label: 'Status',   render: (a) => <Badge tone={a.verified ? 'success' : 'danger'}>{a.verified ? 'Verified' : 'Unverified'}</Badge> },
    { key: 'ip', label: 'IP Address', render: (a) => (
      <div>
        <div style={{ fontWeight: 500 }}>{a.ipAddress || 'N/A'}</div>
        {a.networkProvider && <div style={{ fontSize: '11px', color: 'var(--color-muted)', marginTop: '2px' }}>{a.networkProvider}</div>}
        {a.ipAddress && ipCounts[a.ipAddress] > 1 && <Badge tone="danger">Shared ({ipCounts[a.ipAddress]})</Badge>}
      </div>
    )},
    { key: 'device', label: 'Device',     render: (a) => <span title={a.userAgent}>{parseUA(a.userAgent)}</span> },
    { key: 'face',   label: 'Face Check', render: (a) => <Badge tone={a.faceDetected !== false ? 'success' : 'danger'}>{a.faceDetected !== false ? 'Detected' : 'No Face'}</Badge> },
  ];

  return (
    <div className="container">
      <div className="row">
        <Link to="/sessions" className="btn btn-secondary btn-small">&larr; Back</Link>
      </div>

      <PageHeader title="Session Details" />

      <div className="card">
        <h3 style={{ marginBottom: 'var(--space-3)' }}>Session Info</h3>
        <p><strong>Location:</strong> {session.locationId?.name || 'Unknown'}</p>
        <p style={{ margin: '6px 0' }}><strong>Status:</strong> <Badge tone={statusTone}>{statusLabel}</Badge></p>
        <p><strong>Expires At:</strong> {new Date(session.expiresAt).toLocaleString()}</p>
        <p><strong>Rotations:</strong> {session.rotationCount}</p>
        <div className="form-actions">
          <button className="btn btn-primary" onClick={() => setConfirmAction('rotate')} disabled={!session.isActive}>Rotate Token</button>
          {session.totpEnabled && <Link to={`/sessions/${id}/qr`} className="btn btn-success"><QrCode size={16} />View QR Display</Link>}
          {session.isActive && !isExpired && <button className="btn btn-danger" onClick={() => setConfirmAction('deactivate')}>Deactivate Session</button>}
        </div>
      </div>

      <div className="grid">
        <StatTile label="Total Attendance" value={stats?.totalAttendance ?? 0} />
        <StatTile label="Verified"   value={stats?.verifiedAttendance ?? 0} tone="success" />
        <StatTile label="Unverified" value={(stats?.totalAttendance ?? 0) - (stats?.verifiedAttendance ?? 0)} tone="danger" />
      </div>

      <div className="card card-table">
        <div className="row" style={{ padding: 'var(--space-5) var(--space-6) 0' }}>
          <h3>Attendance Records</h3>
          <button className="btn btn-secondary btn-small" onClick={handleExportCSV}>Export CSV</button>
        </div>
        {attendance.length === 0 ? (
          <p style={{ padding: 'var(--space-6)' }}>No attendance records yet</p>
        ) : (
          <DataTable columns={columns} rows={attendance} rowKey={(a) => a._id} />
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
