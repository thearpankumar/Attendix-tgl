import { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { Flag } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import { SkeletonRows } from '../components/ui/Skeleton';

const FLAG_TONES: Record<string, 'danger' | 'warning' | 'neutral'> = {
  MULTI_STUDENT_DEVICE:   'danger',
  STUDENT_DEVICE_SWITCHED: 'warning',
  RAPID_SUBMISSION:        'warning',
};

interface FlaggedRecord {
  _id: string;
  studentName: string;
  rollNumber: string;
  deviceFlag: string;
  capturedAt: string;
  flagReviewed: boolean;
  sessionId?: { _id: string; description?: string };
}

interface Filter { flagType: string; reviewed: 'all' | 'reviewed' | 'unreviewed'; }

const FlaggedAttendance = () => {
  const [flaggedRecords, setFlaggedRecords] = useState<FlaggedRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState<Filter>({ flagType: '', reviewed: 'all' });

  const fetchData = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (filter.flagType) params.append('flagType', filter.flagType);
      const res = await axios.get<FlaggedRecord[]>(`/api/admin/flagged?${params}`);
      let records = res.data;
      if (filter.reviewed === 'reviewed')   records = records.filter((r) => r.flagReviewed);
      if (filter.reviewed === 'unreviewed') records = records.filter((r) => !r.flagReviewed);
      setFlaggedRecords(records);
    } catch { toast.error('Failed to fetch flagged records'); }
    finally { setLoading(false); }
  }, [filter]);

  useEffect(() => { fetchData(); }, [fetchData]);

  const handleReview = async (id: string, reviewed: boolean) => {
    try {
      await axios.patch(`/api/admin/attendance/${id}/review`, { reviewed });
      toast.success(reviewed ? 'Flag marked as reviewed' : 'Flag review removed');
      fetchData();
    } catch { toast.error('Failed to update review status'); }
  };

  const columns: Column<FlaggedRecord>[] = [
    { key: 'student',  label: 'Student',   render: (r) => r.studentName },
    { key: 'roll',     label: 'Roll No',   render: (r) => r.rollNumber },
    { key: 'flagType', label: 'Flag Type', render: (r) => <Badge tone={FLAG_TONES[r.deviceFlag] || 'neutral'}>{r.deviceFlag?.replace(/_/g, ' ')}</Badge> },
    { key: 'session',  label: 'Session',   render: (r) => (
      <div>
        {r.sessionId?.description || 'Session'}
        <div style={{ color: 'var(--color-faint)', fontSize: '11px' }}>{r.sessionId?._id?.substring(0, 8)}...</div>
      </div>
    )},
    { key: 'time',   label: 'Time',   render: (r) => new Date(r.capturedAt).toLocaleString() },
    { key: 'status', label: 'Status', render: (r) => <Badge tone={r.flagReviewed ? 'success' : 'warning'}>{r.flagReviewed ? 'Reviewed' : 'Pending'}</Badge> },
    { key: 'actions', label: 'Actions', render: (r) => (
      <div className="actions-cell">
        {!r.flagReviewed ? (
          <button className="btn btn-success btn-small" onClick={() => handleReview(r._id, true)}>Mark Reviewed</button>
        ) : (
          <button className="btn btn-secondary btn-small" onClick={() => handleReview(r._id, false)}>Unmark</button>
        )}
      </div>
    )},
  ];

  return (
    <div className="container">
      <PageHeader title="Flagged Attendance Records" />

      <div className="card filter-bar">
        <div>
          <label>Flag Type:</label>
          <select value={filter.flagType} onChange={(e) => setFilter({ ...filter, flagType: e.target.value })}>
            <option value="">All Flags</option>
            <option value="MULTI_STUDENT_DEVICE">Multi-Student Device</option>
            <option value="STUDENT_DEVICE_SWITCHED">Device Switched</option>
            <option value="RAPID_SUBMISSION">Rapid Submission</option>
          </select>
        </div>
        <div>
          <label>Review Status:</label>
          <select value={filter.reviewed} onChange={(e) => setFilter({ ...filter, reviewed: e.target.value as Filter['reviewed'] })}>
            <option value="all">All</option>
            <option value="unreviewed">Unreviewed</option>
            <option value="reviewed">Reviewed</option>
          </select>
        </div>
      </div>

      {loading ? <SkeletonRows /> : flaggedRecords.length === 0 ? (
        <EmptyState icon={Flag} title="All clear" message="No flagged attendance records found." />
      ) : (
        <div className="card card-table">
          <DataTable columns={columns} rows={flaggedRecords} rowKey={(r) => r._id} />
        </div>
      )}

      <div className="info-card">
        <h4>Flag Types Explained</h4>
        <ul>
          <li><strong>Multi-Student Device:</strong> Same device was used by multiple students</li>
          <li><strong>Device Switched:</strong> Student used a different device than their registered device</li>
          <li><strong>Rapid Submission:</strong> Multiple submissions within 10 seconds</li>
        </ul>
      </div>
    </div>
  );
};

export default FlaggedAttendance;
