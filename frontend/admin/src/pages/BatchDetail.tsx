import { useState, useEffect, useRef, useCallback } from 'react';
import { useParams, useNavigate } from 'react-router';
import axios from 'axios';
import { toast } from 'react-toastify';
import { ArrowLeft, Users, Calendar, ClipboardList, ChevronDown, ChevronRight, Download, Search, X } from 'lucide-react';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import InfiniteScrollSentinel from '../components/ui/InfiniteScrollSentinel';
import Button from '../components/ui/Button';
import { SkeletonRows } from '../components/ui/Skeleton';
import { attendanceColor, attendanceColorBg } from '../utils/attendanceColor';
import { downloadBlob, filenameFromContentDisposition } from '../utils/downloadBlob';

const PAGE_SIZE = 25;

interface BatchOverview {
  _id: string;
  name: string;
  description: string | null;
  createdAt: string;
  totalStudents: number;
  totalSessionsInRange: number;
}

interface BatchStudentRow {
  studentId: string;
  name: string;
  rollNumber: string;
  email: string | null;
  present: number;
  totalSessions: number;
  percentage: number;
}

interface StudentSessionRow {
  sessionId: string;
  sessionDate: string;
  status: 'present' | 'absent' | 'unmarked';
  source: 'self_submitted' | 'manual' | null;
  markedByEmail: string | null;
}

/** yyyy-mm-dd (date input value) -> RFC3339 start/end-of-day UTC, or undefined if empty. */
function toRangeStart(value: string): string | undefined {
  return value ? new Date(`${value}T00:00:00Z`).toISOString() : undefined;
}
function toRangeEnd(value: string): string | undefined {
  return value ? new Date(`${value}T23:59:59.999Z`).toISOString() : undefined;
}

const LARGE_RANGE_HINT_THRESHOLD = 200;

/** yyyy-mm-dd for `n` days ago, in local time (matches a date input's value format). */
function daysAgo(n: number): string {
  const d = new Date();
  d.setDate(d.getDate() - n);
  return d.toISOString().slice(0, 10);
}

const BatchDetail = () => {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  // Defaults to the last 30 days rather than all-time: an unbounded query on
  // a batch's full history is the single most expensive request this page
  // can make (measured: a full-history export on a 400-student/200-session
  // batch took ~15x longer than the same export scoped to 10 days), so a
  // bounded default avoids that cost on every first page load. "Clear
  // range" below reverts to all-time when that's actually what's wanted.
  const [dateFromInput, setDateFromInput] = useState(() => daysAgo(30));
  const [dateToInput, setDateToInput] = useState(() => daysAgo(0));

  const [overview, setOverview] = useState<BatchOverview | null>(null);
  const [overviewLoading, setOverviewLoading] = useState(true);

  const [students, setStudents] = useState<BatchStudentRow[]>([]);
  const [studentsLoading, setStudentsLoading] = useState(true);
  const [studentsLoadingMore, setStudentsLoadingMore] = useState(false);
  const [hasMoreStudents, setHasMoreStudents] = useState(true);
  const offsetRef = useRef(0);

  const [searchInput, setSearchInput] = useState('');
  const [search, setSearch] = useState('');
  useEffect(() => {
    if (searchInput === search) return;
    const timeout = setTimeout(() => setSearch(searchInput), 400);
    return () => clearTimeout(timeout);
  }, [searchInput]);

  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [exportingSelected, setExportingSelected] = useState(false);
  const [exportingAll, setExportingAll] = useState(false);

  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [sessionRows, setSessionRows] = useState<Record<string, StudentSessionRow[]>>({});
  const [sessionHasMore, setSessionHasMore] = useState<Record<string, boolean>>({});
  const [sessionLoading, setSessionLoading] = useState<Record<string, boolean>>({});

  const dateFrom = toRangeStart(dateFromInput);
  const dateTo = toRangeEnd(dateToInput);

  const fetchOverview = useCallback(async () => {
    if (!id) return;
    setOverviewLoading(true);
    try {
      const { data } = await axios.get<BatchOverview>(`/api/admin/batches/${id}/overview`, {
        params: { dateFrom, dateTo },
      });
      setOverview(data);
    } catch (_err) {
      toast.error('Failed to load batch overview');
    } finally {
      setOverviewLoading(false);
    }
  }, [id, dateFrom, dateTo]);

  const fetchStudents = useCallback(
    async (reset: boolean) => {
      if (!id) return;
      if (reset) setStudentsLoading(true);
      else setStudentsLoadingMore(true);
      try {
        const offset = reset ? 0 : offsetRef.current;
        const { data } = await axios.get<{ students: BatchStudentRow[]; hasMore: boolean }>(
          `/api/admin/batches/${id}/students`,
          { params: { dateFrom, dateTo, search: search || undefined, limit: PAGE_SIZE, offset } }
        );
        setStudents((prev) => (reset ? data.students : [...prev, ...data.students]));
        offsetRef.current = offset + data.students.length;
        setHasMoreStudents(data.hasMore);
      } catch (_err) {
        toast.error('Failed to load students');
      } finally {
        setStudentsLoading(false);
        setStudentsLoadingMore(false);
      }
    },
    [id, dateFrom, dateTo, search]
  );

  useEffect(() => {
    fetchOverview();
  }, [fetchOverview]);

  // Search or date-range change invalidates both pagination and any
  // checkbox selection (the underlying row set has changed meaning).
  useEffect(() => {
    setSelectedIds(new Set());
    setExpandedId(null);
    fetchStudents(true);
  }, [fetchStudents]);

  const toggleSelect = (studentId: string, checked: boolean) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (checked) next.add(studentId);
      else next.delete(studentId);
      return next;
    });
  };
  const allSelected = students.length > 0 && students.every((s) => selectedIds.has(s.studentId));
  const someSelected = students.some((s) => selectedIds.has(s.studentId)) && !allSelected;
  const toggleSelectAll = () => {
    if (allSelected) {
      setSelectedIds(new Set());
      return;
    }
    setSelectedIds(new Set(students.map((s) => s.studentId)));
  };

  const loadSessionsFor = async (studentId: string, reset: boolean) => {
    if (!id) return;
    setSessionLoading((prev) => ({ ...prev, [studentId]: true }));
    try {
      const offset = reset ? 0 : (sessionRows[studentId]?.length ?? 0);
      const { data } = await axios.get<{ sessions: StudentSessionRow[]; hasMore: boolean }>(
        `/api/admin/batches/${id}/students/${studentId}/sessions`,
        { params: { dateFrom, dateTo, limit: 20, offset } }
      );
      setSessionRows((prev) => ({
        ...prev,
        [studentId]: reset ? data.sessions : [...(prev[studentId] ?? []), ...data.sessions],
      }));
      setSessionHasMore((prev) => ({ ...prev, [studentId]: data.hasMore }));
    } catch (_err) {
      toast.error('Failed to load session detail');
    } finally {
      setSessionLoading((prev) => ({ ...prev, [studentId]: false }));
    }
  };

  const toggleExpand = (studentId: string) => {
    if (expandedId === studentId) {
      setExpandedId(null);
      return;
    }
    setExpandedId(studentId);
    if (!sessionRows[studentId]) loadSessionsFor(studentId, true);
  };

  const runExport = async (scope: 'selected' | 'all') => {
    if (!id) return;
    const setBusy = scope === 'selected' ? setExportingSelected : setExportingAll;
    setBusy(true);
    try {
      const res = await axios.post(
        `/api/admin/batches/${id}/export`,
        { scope, studentIds: scope === 'selected' ? [...selectedIds] : [], dateFrom, dateTo },
        { responseType: 'blob' }
      );
      const filename = filenameFromContentDisposition(res.headers['content-disposition'], `Batch_Export_${id}.xlsx`);
      downloadBlob(new Blob([res.data]), filename);
      toast.success('Export ready');
    } catch (_err) {
      toast.error('Failed to export');
    } finally {
      setBusy(false);
    }
  };

  const columns: Column<BatchStudentRow>[] = [
    {
      key: 'select',
      width: '4%',
      label: (
        <input
          type="checkbox"
          checked={allSelected}
          ref={(el) => {
            if (el) el.indeterminate = someSelected;
          }}
          onChange={toggleSelectAll}
          aria-label="Select all students"
        />
      ),
      render: (s) => (
        <input
          type="checkbox"
          checked={selectedIds.has(s.studentId)}
          onChange={(e) => toggleSelect(s.studentId, e.target.checked)}
          aria-label={`Select ${s.name}`}
        />
      ),
    },
    { key: 'name', label: 'Name', render: (s) => s.name },
    { key: 'rollNumber', label: 'Roll Number', render: (s) => s.rollNumber },
    { key: 'email', label: 'Email', render: (s) => s.email || '—' },
    {
      key: 'percentage',
      label: 'Attendance %',
      render: (s) => (
        <span className="percent-pill" style={{ color: attendanceColor(s.percentage), background: attendanceColorBg(s.percentage) }}>
          {s.percentage.toFixed(1)}% <small>({s.present}/{s.totalSessions})</small>
        </span>
      ),
    },
    {
      key: 'expand',
      label: '',
      width: '6%',
      render: (s) => (
        <button className="btn btn-secondary btn-small" onClick={() => toggleExpand(s.studentId)} title="View session-wise detail">
          {expandedId === s.studentId ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>
      ),
    },
  ];

  return (
    <div className="container">
      <div className="page-header" style={{ marginBottom: 'var(--space-5)', gap: 'var(--space-3)' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }}>
          <button className="btn btn-secondary btn-small" style={{ width: 'fit-content', marginBottom: 4 }} onClick={() => navigate('/batches')}>
            <ArrowLeft size={14} style={{ marginRight: 6 }} /> Back to Batches
          </button>
          <h1 className="page-title" style={{ fontSize: '1.9rem', fontWeight: 800, margin: 0 }}>
            {overview?.name || 'Loading…'}
          </h1>
          {overview?.description && (
            <p className="page-subtitle" style={{ margin: 0, color: 'var(--color-muted)' }}>{overview.description}</p>
          )}
        </div>
      </div>

      {/* Unified control panel: compact overview stats + search/filters/export,
          all in one place instead of a separate full-height Overview screen. */}
      <div className="card" style={{ marginBottom: 'var(--space-4)', display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }}>
        <div className="meta-bar">
          {overviewLoading ? (
            <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-muted)' }}>Loading batch stats…</span>
          ) : overview ? (
            <>
              <span className="meta-item">
                <span className="meta-item-icon"><Users size={14} /></span>
                <strong>{overview.totalStudents.toLocaleString()}</strong>&nbsp;students
              </span>
              <span className="meta-divider" />
              <span className="meta-item">
                <span className="meta-item-icon"><ClipboardList size={14} /></span>
                <strong>{overview.totalSessionsInRange.toLocaleString()}</strong>&nbsp;sessions in range
              </span>
              <span className="meta-divider" />
              <span className="meta-item">
                <span className="meta-item-icon"><Calendar size={14} /></span>
                Created&nbsp;<strong>{new Date(overview.createdAt).toLocaleDateString()}</strong>
              </span>
              {overview.totalSessionsInRange > LARGE_RANGE_HINT_THRESHOLD && (
                <span className="meta-item">
                  <span className="meta-item-icon tone-warning"><ClipboardList size={14} /></span>
                  Narrow the date range for faster loading
                </span>
              )}
            </>
          ) : (
            <span style={{ fontSize: 'var(--text-sm)', color: 'var(--color-danger)' }}>Failed to load batch overview.</span>
          )}
        </div>

        <div style={{ height: 1, background: 'var(--color-border)' }} />

        <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 'var(--space-3)' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-2)', flex: '1 1 240px', minWidth: 200, background: 'var(--color-bg-subtle)', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)', padding: '7px 12px' }}>
            <Search size={15} className="text-muted" />
            <input
              type="text"
              value={searchInput}
              onChange={(e) => setSearchInput(e.target.value)}
              placeholder="Search this batch's students by name or roll number…"
              style={{ flex: 1, border: 'none', outline: 'none', background: 'transparent', fontSize: 'var(--text-sm)', color: 'var(--color-text)' }}
            />
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-2)' }}>
            <label style={{ fontSize: 'var(--text-xs)', color: 'var(--color-muted)' }}>From</label>
            <input type="date" value={dateFromInput} onChange={(e) => setDateFromInput(e.target.value)} />
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 'var(--space-2)' }}>
            <label style={{ fontSize: 'var(--text-xs)', color: 'var(--color-muted)' }}>To</label>
            <input type="date" value={dateToInput} onChange={(e) => setDateToInput(e.target.value)} />
          </div>
          {(dateFromInput || dateToInput) && (
            <button className="btn btn-secondary btn-small" onClick={() => { setDateFromInput(''); setDateToInput(''); }} title="Clear date range">
              <X size={13} />
            </button>
          )}
          <div style={{ flex: 1 }} />
          <Button variant="secondary" size="sm" onClick={() => runExport('selected')} disabled={selectedIds.size === 0 || exportingSelected}>
            <Download size={14} style={{ marginRight: 6, verticalAlign: -2 }} />
            {exportingSelected ? 'Exporting…' : `Export Selected (${selectedIds.size})`}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => runExport('all')} disabled={exportingAll}>
            <Download size={14} style={{ marginRight: 6, verticalAlign: -2 }} />
            {exportingAll ? 'Exporting…' : 'Export Full Batch'}
          </Button>
        </div>
      </div>

      <div className="card card-table">
        {studentsLoading ? (
          <SkeletonRows count={5} />
        ) : students.length === 0 ? (
          <div className="empty-state">
            <Users size={48} className="empty-icon" />
            <h3>No Students Found</h3>
            {search && <p>No students match "{search}" in this batch.</p>}
          </div>
        ) : (
          <>
            <DataTable
              rows={students}
              columns={columns}
              rowKey={(s) => s.studentId}
              expandedRowKey={expandedId}
              renderExpandedRow={(s) => (
                <div style={{ padding: 'var(--space-4) var(--space-5)', background: 'var(--color-bg-subtle)' }}>
                  <h4 style={{ margin: '0 0 var(--space-3) 0' }}>
                    {s.name} ({s.rollNumber}) — Session Detail
                  </h4>
                  {sessionLoading[s.studentId] && !sessionRows[s.studentId] ? (
                    <SkeletonRows count={3} />
                  ) : !sessionRows[s.studentId] || sessionRows[s.studentId].length === 0 ? (
                    <p style={{ color: 'var(--color-muted)', margin: 0 }}>No sessions in the selected date range.</p>
                  ) : (
                    <>
                      <table className="table" style={{ width: '100%' }}>
                        <thead>
                          <tr>
                            <th style={{ textAlign: 'left' }}>Date</th>
                            <th style={{ textAlign: 'left' }}>Status</th>
                            <th style={{ textAlign: 'left' }}>Source</th>
                            <th style={{ textAlign: 'left' }}>Marked By</th>
                          </tr>
                        </thead>
                        <tbody>
                          {sessionRows[s.studentId].map((row) => (
                            <tr key={row.sessionId}>
                              <td>{new Date(row.sessionDate).toLocaleString()}</td>
                              <td style={{ textTransform: 'capitalize' }}>{row.status}</td>
                              <td>{row.source ? row.source.replace('_', ' ') : '—'}</td>
                              <td>{row.markedByEmail || '—'}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                      {sessionHasMore[s.studentId] && (
                        <button
                          className="btn btn-secondary btn-small"
                          style={{ marginTop: 12 }}
                          onClick={() => loadSessionsFor(s.studentId, false)}
                          disabled={sessionLoading[s.studentId]}
                        >
                          {sessionLoading[s.studentId] ? 'Loading…' : 'Load more sessions'}
                        </button>
                      )}
                    </>
                  )}
                </div>
              )}
            />
            {hasMoreStudents && (
              <>
                <InfiniteScrollSentinel onIntersect={() => fetchStudents(false)} disabled={studentsLoadingMore} />
                {studentsLoadingMore && <div className="load-more-indicator">Loading more…</div>}
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
};

export default BatchDetail;
