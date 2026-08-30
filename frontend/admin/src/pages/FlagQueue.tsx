import { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { Flag } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import Modal from '../components/ui/Modal';
import { SkeletonRows } from '../components/ui/Skeleton';

type Severity = 'high' | 'medium' | 'low';

const SEVERITY_TONES: Record<Severity, 'danger' | 'warning' | 'neutral'> = {
  high: 'danger',
  medium: 'warning',
  low: 'neutral',
};

interface FlaggedRecord {
  _id: string;
  sessionId: string;
  rollNumber: string;
  studentName: string;
  capturedAt: string;
  flagged: boolean;
  flagReason: string | null;
  flagReviewed: boolean;
  flagSeverity: Severity | null;
  reviewNotes: string | null;
  gpsAnomalies: { type: string; severity: Severity; details?: string }[];
  emulatorDetected: boolean;
  emulatorFlags: { type: string; severity: Severity; details?: string }[];
  integrityChecks: { type: string; passed: boolean; details?: string }[];
}

interface QueuePage {
  items: FlaggedRecord[];
  total: number;
  page: number;
  pageSize: number;
}

interface Filter {
  reviewed: 'all' | 'reviewed' | 'unreviewed';
  severity: 'all' | Severity;
  sessionId: string;
}

const PAGE_SIZE = 25;

const FlagQueue = () => {
  const [data, setData] = useState<QueuePage | null>(null);
  const [loading, setLoading] = useState(true);
  const [page, setPage] = useState(1);
  const [filter, setFilter] = useState<Filter>({ reviewed: 'unreviewed', severity: 'all', sessionId: '' });
  const [selected, setSelected] = useState<Set<string>>(new Set());

  // Single-row and bulk review both funnel through this: {ids, action} —
  // review notes are mandatory on the backend, so both paths open the same
  // notes modal rather than firing the request immediately.
  const [reviewTarget, setReviewTarget] = useState<{ ids: string[]; action: 'approve' | 'reject' } | null>(null);
  const [reviewNotes, setReviewNotes] = useState('');
  const [submittingReview, setSubmittingReview] = useState(false);

  const fetchData = useCallback(async () => {
    setLoading(true);
    try {
      const params = new URLSearchParams({ page: String(page), pageSize: String(PAGE_SIZE) });
      if (filter.reviewed !== 'all') params.set('reviewed', String(filter.reviewed === 'reviewed'));
      if (filter.severity !== 'all') params.set('severity', filter.severity);
      if (filter.sessionId.trim()) params.set('sessionId', filter.sessionId.trim());
      const res = await axios.get<QueuePage>(`/api/admin/security/flags/queue?${params}`);
      setData(res.data);
      setSelected(new Set());
    } catch {
      toast.error('Failed to fetch flagged records');
    } finally {
      setLoading(false);
    }
  }, [page, filter]);

  useEffect(() => { fetchData(); }, [fetchData]);

  // Any filter change should reset to page 1 — otherwise a narrower filter
  // can land on an out-of-range page showing nothing with no obvious reason.
  const updateFilter = (next: Partial<Filter>) => {
    setFilter((f) => ({ ...f, ...next }));
    setPage(1);
  };

  const openReview = (ids: string[], action: 'approve' | 'reject') => {
    setReviewTarget({ ids, action });
    setReviewNotes('');
  };

  const closeReview = () => {
    setReviewTarget(null);
    setReviewNotes('');
  };

  const submitReview = async () => {
    if (!reviewTarget || !reviewNotes.trim()) return;
    setSubmittingReview(true);
    try {
      if (reviewTarget.ids.length === 1) {
        await axios.post(`/api/admin/security/attendance/${reviewTarget.ids[0]}/review`, {
          action: reviewTarget.action,
          notes: reviewNotes.trim(),
        });
      } else {
        await axios.post('/api/admin/security/flags/bulk-review', {
          ids: reviewTarget.ids,
          action: reviewTarget.action,
          notes: reviewNotes.trim(),
        });
      }
      toast.success(reviewTarget.action === 'approve' ? 'Marked safe' : 'Rejected');
      closeReview();
      fetchData();
    } catch {
      toast.error('Failed to submit review');
    } finally {
      setSubmittingReview(false);
    }
  };

  const toggleSelected = (id: string) => {
    setSelected((s) => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    if (!data) return;
    setSelected((s) =>
      s.size === data.items.length ? new Set() : new Set(data.items.map((r) => r._id))
    );
  };

  const columns: Column<FlaggedRecord>[] = [
    {
      key: 'select',
      label: (
        <input
          type="checkbox"
          checked={!!data && data.items.length > 0 && selected.size === data.items.length}
          onChange={toggleSelectAll}
          aria-label="Select all"
        />
      ),
      width: '36px',
      render: (r) => (
        <input
          type="checkbox"
          checked={selected.has(r._id)}
          onChange={() => toggleSelected(r._id)}
          aria-label={`Select ${r.rollNumber}`}
        />
      ),
    },
    {
      key: 'severity',
      label: 'Severity',
      render: (r) => r.flagSeverity ? <Badge tone={SEVERITY_TONES[r.flagSeverity]}>{r.flagSeverity}</Badge> : <Badge tone="neutral">—</Badge>,
    },
    { key: 'student', label: 'Student', render: (r) => r.studentName },
    { key: 'roll', label: 'Roll No', render: (r) => r.rollNumber },
    {
      key: 'reason',
      label: 'Flags',
      render: (r) => {
        const types = [
          ...r.gpsAnomalies.map((a) => a.type),
          ...r.emulatorFlags.map((f) => f.type),
          ...r.integrityChecks.filter((c) => !c.passed).map((c) => c.type),
        ];
        if (types.length === 0) return r.flagReason ?? '—';
        return (
          <div style={{ display: 'flex', gap: '4px', flexWrap: 'wrap', justifyContent: 'center' }}>
            {types.slice(0, 3).map((t, i) => (
              <span key={i} style={{ fontSize: '11px', color: 'var(--color-muted)' }}>{t.replace(/_/g, ' ')}</span>
            ))}
            {types.length > 3 && <span style={{ fontSize: '11px', color: 'var(--color-faint)' }}>+{types.length - 3} more</span>}
          </div>
        );
      },
    },
    { key: 'session', label: 'Session', render: (r) => <span style={{ color: 'var(--color-faint)', fontSize: '11px' }}>{r.sessionId.substring(0, 8)}...</span> },
    { key: 'time', label: 'Time', render: (r) => new Date(r.capturedAt).toLocaleString() },
    {
      key: 'status',
      label: 'Status',
      render: (r) => r.flagReviewed
        ? <Badge tone="success">Reviewed</Badge>
        : <Badge tone="warning">Pending</Badge>,
    },
    {
      key: 'actions',
      label: 'Actions',
      render: (r) => (
        <div className="actions-cell">
          {!r.flagReviewed ? (
            <>
              <button className="btn btn-success btn-small" onClick={() => openReview([r._id], 'approve')}>Approve</button>
              <button className="btn btn-danger btn-small" onClick={() => openReview([r._id], 'reject')}>Reject</button>
            </>
          ) : (
            <span style={{ fontSize: '12px', color: 'var(--color-muted)' }} title={r.reviewNotes ?? ''}>
              {r.reviewNotes ? (r.reviewNotes.length > 40 ? `${r.reviewNotes.slice(0, 40)}...` : r.reviewNotes) : 'No notes'}
            </span>
          )}
        </div>
      ),
    },
  ];

  const totalPages = data ? Math.max(1, Math.ceil(data.total / data.pageSize)) : 1;

  return (
    <div className="container">
      <PageHeader title="Flagged Attendance Queue" />

      <div className="card filter-bar">
        <div>
          <label>Review Status:</label>
          <select value={filter.reviewed} onChange={(e) => updateFilter({ reviewed: e.target.value as Filter['reviewed'] })}>
            <option value="unreviewed">Unreviewed</option>
            <option value="reviewed">Reviewed</option>
            <option value="all">All</option>
          </select>
        </div>
        <div>
          <label>Severity:</label>
          <select value={filter.severity} onChange={(e) => updateFilter({ severity: e.target.value as Filter['severity'] })}>
            <option value="all">All</option>
            <option value="high">High</option>
            <option value="medium">Medium</option>
            <option value="low">Low</option>
          </select>
        </div>
        <div>
          <label>Session ID:</label>
          <input
            type="text"
            placeholder="Leave blank for all sessions"
            value={filter.sessionId}
            onChange={(e) => updateFilter({ sessionId: e.target.value })}
            style={{ minWidth: '220px' }}
          />
        </div>
      </div>

      {selected.size > 0 && (
        <div className="card filter-bar" style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
          <span>{selected.size} selected</span>
          <button className="btn btn-success btn-small" onClick={() => openReview([...selected], 'approve')}>Approve selected</button>
          <button className="btn btn-danger btn-small" onClick={() => openReview([...selected], 'reject')}>Reject selected</button>
        </div>
      )}

      {loading ? <SkeletonRows /> : !data || data.items.length === 0 ? (
        <EmptyState icon={Flag} title="All clear" message="No flagged attendance records match these filters." />
      ) : (
        <>
          <div className="card card-table">
            <DataTable columns={columns} rows={data.items} rowKey={(r) => r._id} />
          </div>
          <div className="filter-bar" style={{ justifyContent: 'space-between', alignItems: 'center' }}>
            <span style={{ color: 'var(--color-muted)', fontSize: '13px' }}>
              {data.total} total &middot; page {data.page} of {totalPages}
            </span>
            <div style={{ display: 'flex', gap: '8px' }}>
              <button className="btn btn-secondary btn-small" disabled={page <= 1} onClick={() => setPage((p) => p - 1)}>Previous</button>
              <button className="btn btn-secondary btn-small" disabled={page >= totalPages} onClick={() => setPage((p) => p + 1)}>Next</button>
            </div>
          </div>
        </>
      )}

      <Modal
        open={!!reviewTarget}
        onClose={closeReview}
        title={(() => {
          if (!reviewTarget) return '';
          const verb = reviewTarget.action === 'approve' ? 'Approve' : 'Reject';
          const noun = reviewTarget.ids.length > 1 ? `${reviewTarget.ids.length} records` : 'record';
          return `${verb} ${noun}`;
        })()}
        footer={
          <>
            <button type="button" className="btn btn-secondary" onClick={closeReview} disabled={submittingReview}>Cancel</button>
            <button
              type="button"
              className={reviewTarget?.action === 'approve' ? 'btn btn-success' : 'btn btn-danger'}
              onClick={submitReview}
              disabled={submittingReview || !reviewNotes.trim()}
            >
              {submittingReview ? 'Submitting...' : 'Confirm'}
            </button>
          </>
        }
      >
        <p style={{ margin: '0 0 12px 0', color: 'var(--color-muted)', fontSize: '14px' }}>
          Review notes are required — briefly note why {reviewTarget && reviewTarget.ids.length > 1 ? 'these submissions are' : 'this submission is'} being {reviewTarget?.action === 'approve' ? 'approved' : 'rejected'}.
        </p>
        <textarea
          autoFocus
          rows={3}
          style={{ width: '100%', resize: 'vertical' }}
          placeholder="e.g. Verified with student, GPS drift from a known dead zone"
          value={reviewNotes}
          onChange={(e) => setReviewNotes(e.target.value)}
        />
      </Modal>
    </div>
  );
};

export default FlagQueue;
