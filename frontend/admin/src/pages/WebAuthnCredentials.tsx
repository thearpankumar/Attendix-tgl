import { useState, useEffect } from 'react';
import { toast } from 'react-toastify';
import axios from 'axios';
import { Fingerprint } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import StatTile from '../components/ui/StatTile';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import Badge from '../components/ui/Badge';
import EmptyState from '../components/ui/EmptyState';
import { SkeletonTiles, SkeletonRows } from '../components/ui/Skeleton';


interface Credential {
  _id: string;
  studentId: string;
  deviceLabel?: string;
  enrolledAt: string;
  lastUsedAt?: string;
  isSuspended: boolean;
}

interface Stats {
  totalEnrolled: number;
  active: number;
  suspended: number;
  deviceTypes?: Record<string, number>;
  enrollmentTrends?: { last7Days: number };
}

interface Pagination { pages: number; total: number; }

function WebAuthnCredentials() {
  const [credentials, setCredentials] = useState<Credential[]>([]);
  const [stats, setStats] = useState<Stats | null>(null);
  const [loading, setLoading] = useState(true);
  const [search, setSearch] = useState('');
  const [suspendedFilter, setSuspendedFilter] = useState('all');
  const [page, setPage] = useState(1);
  const [pagination, setPagination] = useState<Pagination>({ pages: 1, total: 0 });
  const [showResetModal, setShowResetModal] = useState(false);
  const [showSuspendModal, setShowSuspendModal] = useState(false);
  const [selectedStudent, setSelectedStudent] = useState<Credential | null>(null);
  const [reason, setReason] = useState('');
  const [actionLoading, setActionLoading] = useState(false);


  useEffect(() => { fetchCredentials(); fetchStats(); }, [search, suspendedFilter, page]);

  const fetchCredentials = async () => {
    setLoading(true);
    try {
      const params = new URLSearchParams({ page: String(page), limit: '20' });
      if (search) params.append('search', search);
      if (suspendedFilter !== 'all') params.append('suspended', suspendedFilter);
      const res = await axios.get<{ credentials: Credential[], pagination: Pagination }>(`/api/admin/webauthn/credentials?${params}`);
      setCredentials(res.data.credentials);
      setPagination(res.data.pagination);
    } catch (error) { 
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to fetch credentials'); 
    }
    finally { setLoading(false); }
  };

  const fetchStats = async () => {
    try {
      const res = await axios.get<Stats>('/api/admin/webauthn/stats');
      setStats(res.data);
    } catch { /* silently fail */ }
  };

  const handleReset = async () => {
    if (!reason.trim()) { toast.error('Please provide a reason'); return; }
    setActionLoading(true);
    try {
      const res = await axios.post<{ message: string }>('/api/admin/webauthn/reset', { 
        rollNumber: selectedStudent?.studentId, 
        reason: reason.trim() 
      });
      toast.success(res.data.message); 
      setShowResetModal(false); 
      setSelectedStudent(null); 
      setReason(''); 
      fetchCredentials(); 
      fetchStats();
    } catch (error) { 
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to reset credential'); 
    }
    finally { setActionLoading(false); }
  };

  const handleSuspend = async (suspend: boolean) => {
    if (!reason.trim()) { toast.error('Please provide a reason'); return; }
    setActionLoading(true);
    try {
      const res = await axios.post<{ message: string }>(`/api/admin/webauthn/${suspend ? 'suspend' : 'unsuspend'}`, { 
        rollNumber: selectedStudent?.studentId, 
        reason: reason.trim() 
      });
      toast.success(res.data.message); 
      setShowSuspendModal(false); 
      setSelectedStudent(null); 
      setReason(''); 
      fetchCredentials(); 
      fetchStats();
    } catch (error) { 
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || `Failed to ${suspend ? 'suspend' : 'unsuspend'} credential`); 
    }
    finally { setActionLoading(false); }
  };

  const fmt = (d: string) => new Date(d).toLocaleString();
  const deviceTypeEntries = stats ? Object.entries(stats.deviceTypes || {}) : [];
  const maxDeviceCount = Math.max(1, ...deviceTypeEntries.map(([, c]) => c));

  const columns: Column<Credential>[] = [
    { key: 'studentId', label: 'Roll Number', render: (c) => <strong>{c.studentId}</strong> },
    { key: 'device',    label: 'Device',      render: (c) => c.deviceLabel || 'Unknown' },
    { key: 'enrolled',  label: 'Enrolled',    render: (c) => fmt(c.enrolledAt) },
    { key: 'lastUsed',  label: 'Last Used',   render: (c) => c.lastUsedAt ? fmt(c.lastUsedAt) : 'Never' },
    { key: 'status',    label: 'Status',      render: (c) => <Badge tone={c.isSuspended ? 'danger' : 'success'}>{c.isSuspended ? 'Suspended' : 'Active'}</Badge> },
    { key: 'actions',   label: 'Actions',     render: (c) => (
      <div className="actions-cell">
        <button className="btn btn-secondary btn-small" onClick={() => { setSelectedStudent(c); setShowResetModal(true); }}>Reset</button>
        <button className={c.isSuspended ? 'btn btn-success btn-small' : 'btn btn-danger btn-small'} onClick={() => { setSelectedStudent(c); setShowSuspendModal(true); }}>
          {c.isSuspended ? 'Unsuspend' : 'Suspend'}
        </button>
      </div>
    )},
  ];

  return (
    <div className="container">
      <PageHeader title="Biometric Devices" />

      {loading && !stats ? <SkeletonTiles count={4} /> : stats ? (
        <>
          <div className="grid">
            <StatTile label="Total Enrolled"        value={stats.totalEnrolled}                    icon={Fingerprint} />
            <StatTile label="Active"                value={stats.active}                           tone="success" />
            <StatTile label="Suspended"             value={stats.suspended}                        tone="danger" />
            <StatTile label="Enrolled (Last 7 Days)" value={stats.enrollmentTrends?.last7Days || 0} />
          </div>
          {deviceTypeEntries.length > 0 && (
            <div className="card chart-card">
              <h4>Device Types</h4>
              {deviceTypeEntries.map(([type, count]) => (
                <div key={type} className="device-breakdown-row">
                  <span className="device-breakdown-label">{type}</span>
                  <div className="device-breakdown-track"><div className="device-breakdown-fill" style={{ width: `${(count / maxDeviceCount) * 100}%` }} /></div>
                  <span className="device-breakdown-count">{count}</span>
                </div>
              ))}
            </div>
          )}
        </>
      ) : null}

      <div className="card filter-bar">
        <input type="text" placeholder="Search by roll number..." value={search} onChange={(e) => { setSearch(e.target.value); setPage(1); }} style={{ flex: 1, minWidth: '200px', padding: '10px', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)' }} />
        <select value={suspendedFilter} onChange={(e) => { setSuspendedFilter(e.target.value); setPage(1); }}>
          <option value="all">All Status</option>
          <option value="false">Active Only</option>
          <option value="true">Suspended Only</option>
        </select>
      </div>

      <div className="card card-table">
        {loading ? <SkeletonRows count={4} /> : credentials.length === 0 ? (
          <EmptyState icon={Fingerprint} title="No credentials found" />
        ) : (
          <>
            <DataTable columns={columns} rows={credentials} rowKey={(c) => c._id} />
            {pagination.pages > 1 && (
              <div style={{ marginTop: '15px', display: 'flex', justifyContent: 'center', gap: '5px', alignItems: 'center' }}>
                <button className="btn btn-secondary btn-small" disabled={page === 1} onClick={() => setPage((p) => p - 1)}>Previous</button>
                <span style={{ padding: '0 10px', fontSize: 'var(--text-sm)' }}>Page {page} of {pagination.pages}</span>
                <button className="btn btn-secondary btn-small" disabled={page === pagination.pages} onClick={() => setPage((p) => p + 1)}>Next</button>
              </div>
            )}
          </>
        )}
      </div>

      <Modal open={showResetModal} onClose={() => setShowResetModal(false)} title="Reset Biometric Credential">
        <p style={{ margin: '0 0 15px' }}>This will delete the biometric credential for <strong>{selectedStudent?.studentId}</strong>. The student will need to re-enroll.</p>
        <div className="form-group">
          <label>Reason (required)</label>
          <textarea value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Enter reason for reset..." rows={3} />
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" onClick={handleReset} disabled={actionLoading}>{actionLoading ? 'Resetting...' : 'Reset Credential'}</button>
          <button className="btn btn-secondary" onClick={() => setShowResetModal(false)}>Cancel</button>
        </div>
      </Modal>

      <Modal open={showSuspendModal} onClose={() => setShowSuspendModal(false)} title={`${selectedStudent?.isSuspended ? 'Unsuspend' : 'Suspend'} Credential`}>
        <p style={{ margin: '0 0 15px' }}>
          {selectedStudent?.isSuspended
            ? `This will unsuspend the credential for ${selectedStudent.studentId}.`
            : `This will suspend the credential for ${selectedStudent?.studentId}.`}
        </p>
        <div className="form-group">
          <label>Reason (required)</label>
          <textarea value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Enter reason..." rows={3} />
        </div>
        <div className="form-actions">
          <button className="btn btn-primary" onClick={() => handleSuspend(!selectedStudent?.isSuspended)} disabled={actionLoading}>
            {actionLoading ? 'Processing...' : selectedStudent?.isSuspended ? 'Unsuspend' : 'Suspend'}
          </button>
          <button className="btn btn-secondary" onClick={() => setShowSuspendModal(false)}>Cancel</button>
        </div>
      </Modal>
    </div>
  );
}

export default WebAuthnCredentials;
