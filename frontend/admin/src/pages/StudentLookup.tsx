import { useState, useRef, useEffect, useCallback } from 'react';
import { useNavigate } from 'react-router';
import axios from 'axios';
import { toast } from 'react-toastify';
import { Search, Upload, FileSpreadsheet, Download, ExternalLink, ChevronDown, ChevronRight } from 'lucide-react';
import DataTable from '../components/ui/DataTable';
import type { Column } from '../components/ui/DataTable';
import InfiniteScrollSentinel from '../components/ui/InfiniteScrollSentinel';
import { SkeletonRows } from '../components/ui/Skeleton';
import { attendanceColor, attendanceColorBg } from '../utils/attendanceColor';
import { downloadBlob, filenameFromContentDisposition } from '../utils/downloadBlob';

const PAGE_SIZE = 25;

interface AllStudentRow {
  studentId: string;
  name: string;
  rollNumber: string;
  email: string | null;
  batchId: string;
  batchName: string;
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

interface LookupMatch {
  rollNumberQueried: string;
  studentId: string;
  batchId: string;
  batchName: string;
  name: string;
  email: string | null;
  present: number;
  totalSessions: number;
  percentage: number;
}

interface RollNumberAggregate {
  rollNumber: string;
  totalPresent: number;
  totalSessions: number;
  percentage: number;
  matchedBatchCount: number;
}

interface StudentLookupResponse {
  matches: LookupMatch[];
  aggregates: RollNumberAggregate[];
  notFound: string[];
}

/** yyyy-mm-dd (date input value) -> RFC3339 start/end-of-day UTC, or undefined if empty. */
function toRangeStart(value: string): string | undefined {
  return value ? new Date(`${value}T00:00:00Z`).toISOString() : undefined;
}
function toRangeEnd(value: string): string | undefined {
  return value ? new Date(`${value}T23:59:59.999Z`).toISOString() : undefined;
}

const StudentLookup = () => {
  const navigate = useNavigate();

  // ── Shared date range (applies to both the direct list below and the
  // bulk roll-number lookup further down the page) ──
  const [dateFromInput, setDateFromInput] = useState('');
  const [dateToInput, setDateToInput] = useState('');
  const dateFrom = toRangeStart(dateFromInput);
  const dateTo = toRangeEnd(dateToInput);

  // ── Direct student list: the "Student View" — every student across
  // every batch this admin can see, browsable without needing to already
  // know a roll number. ──
  const [students, setStudents] = useState<AllStudentRow[]>([]);
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

  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [sessionRows, setSessionRows] = useState<Record<string, StudentSessionRow[]>>({});
  const [sessionHasMore, setSessionHasMore] = useState<Record<string, boolean>>({});
  const [sessionLoading, setSessionLoading] = useState<Record<string, boolean>>({});

  const fetchStudents = useCallback(
    async (reset: boolean) => {
      if (reset) setStudentsLoading(true);
      else setStudentsLoadingMore(true);
      try {
        const offset = reset ? 0 : offsetRef.current;
        const { data } = await axios.get<{ students: AllStudentRow[]; hasMore: boolean }>('/api/admin/students', {
          params: { dateFrom, dateTo, search: search || undefined, limit: PAGE_SIZE, offset },
        });
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
    [dateFrom, dateTo, search]
  );

  useEffect(() => {
    setExpandedId(null);
    fetchStudents(true);
  }, [fetchStudents]);

  const loadSessionsFor = async (row: AllStudentRow, reset: boolean) => {
    setSessionLoading((prev) => ({ ...prev, [row.studentId]: true }));
    try {
      const offset = reset ? 0 : (sessionRows[row.studentId]?.length ?? 0);
      const { data } = await axios.get<{ sessions: StudentSessionRow[]; hasMore: boolean }>(
        `/api/admin/batches/${row.batchId}/students/${row.studentId}/sessions`,
        { params: { dateFrom, dateTo, limit: 20, offset } }
      );
      setSessionRows((prev) => ({
        ...prev,
        [row.studentId]: reset ? data.sessions : [...(prev[row.studentId] ?? []), ...data.sessions],
      }));
      setSessionHasMore((prev) => ({ ...prev, [row.studentId]: data.hasMore }));
    } catch (_err) {
      toast.error('Failed to load session detail');
    } finally {
      setSessionLoading((prev) => ({ ...prev, [row.studentId]: false }));
    }
  };

  const toggleExpand = (row: AllStudentRow) => {
    if (expandedId === row.studentId) {
      setExpandedId(null);
      return;
    }
    setExpandedId(row.studentId);
    if (!sessionRows[row.studentId]) loadSessionsFor(row, true);
  };

  const columns: Column<AllStudentRow>[] = [
    { key: 'name', label: 'Name', render: (s) => s.name },
    { key: 'rollNumber', label: 'Roll Number', render: (s) => s.rollNumber },
    {
      key: 'batch',
      label: 'Batch',
      render: (s) => (
        <button className="btn btn-secondary btn-small" onClick={() => navigate(`/batches/${s.batchId}`)} title="Open batch">
          {s.batchName} <ExternalLink size={11} style={{ marginLeft: 4, verticalAlign: -1 }} />
        </button>
      ),
    },
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
        <button className="btn btn-secondary btn-small" onClick={() => toggleExpand(s)} title="View session-wise detail">
          {expandedId === s.studentId ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>
      ),
    },
  ];

  // ── Bulk lookup by roll number(s) — paste or upload, for when you already
  // know which roll numbers you're after (e.g. re-enrolled students who
  // appear in more than one batch). ──
  const [rollNumbersText, setRollNumbersText] = useState('');
  const [selectedFile, setSelectedFile] = useState<File | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [searching, setSearching] = useState(false);
  const [result, setResult] = useState<StudentLookupResponse | null>(null);
  const [exporting, setExporting] = useState(false);

  const validateAndSetFile = (file: File) => {
    const nameLower = file.name.toLowerCase();
    if (!['.csv', '.xlsx', '.xls', '.ods'].some((ext) => nameLower.endsWith(ext))) {
      toast.error('Please upload a valid .csv, .xlsx, .xls, or .ods file');
      return;
    }
    if (file.size > 10 * 1024 * 1024) {
      toast.error('File size must be less than 10MB');
      return;
    }
    setSelectedFile(file);
    setRollNumbersText('');
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const file = e.dataTransfer.files[0];
    if (file) validateAndSetFile(file);
  };

  const handleBulkSearch = async () => {
    setSearching(true);
    setResult(null);
    try {
      if (selectedFile) {
        const data = new FormData();
        data.append('file', selectedFile);
        if (dateFrom) data.append('dateFrom', dateFrom);
        if (dateTo) data.append('dateTo', dateTo);
        const res = await axios.post<StudentLookupResponse>('/api/admin/students/lookup/file', data, {
          headers: { 'Content-Type': 'multipart/form-data' },
        });
        setResult(res.data);
      } else {
        const rollNumbers = rollNumbersText
          .split(/[,\n]/)
          .map((r) => r.trim())
          .filter((r) => r.length > 0);
        if (rollNumbers.length === 0) {
          toast.error('Enter at least one roll number, or upload a file');
          setSearching(false);
          return;
        }
        const res = await axios.post<StudentLookupResponse>('/api/admin/students/lookup', {
          rollNumbers,
          dateFrom,
          dateTo,
        });
        setResult(res.data);
      }
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Lookup failed');
    } finally {
      setSearching(false);
    }
  };

  const handleBulkExport = async () => {
    if (!result || result.matches.length === 0) return;
    setExporting(true);
    try {
      const studentIds = result.matches.map((m) => m.studentId);
      const res = await axios.post(
        '/api/admin/students/lookup/export',
        { studentIds, notFound: result.notFound, dateFrom, dateTo },
        { responseType: 'blob' }
      );
      const filename = filenameFromContentDisposition(res.headers['content-disposition'], 'Student_Lookup_Export.xlsx');
      downloadBlob(new Blob([res.data]), filename);
      toast.success('Export ready');
    } catch (_err) {
      toast.error('Failed to export');
    } finally {
      setExporting(false);
    }
  };

  return (
    <div className="container">
      <div className="page-header" style={{ marginBottom: '1.5rem' }}>
        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
          <h1 className="page-title" style={{ fontSize: '2.2rem', fontWeight: 800, margin: 0 }}>Students</h1>
          <p className="page-subtitle" style={{ margin: 0, color: 'var(--text-muted)' }}>
            Every student across every batch you can see, with attendance %. Search below, or look up specific roll numbers further down.
          </p>
        </div>
      </div>

      <div className="card" style={{ marginBottom: '1rem', padding: '12px 16px', display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: 12 }}>
        <Search size={16} className="text-muted" />
        <input
          type="text"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          placeholder="Search by name, roll number, or batch…"
          style={{ flex: 1, minWidth: 200, border: 'none', outline: 'none', background: 'transparent', fontSize: '0.95rem', color: 'var(--text-color)' }}
        />
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <label style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>From</label>
          <input type="date" value={dateFromInput} onChange={(e) => setDateFromInput(e.target.value)} />
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <label style={{ fontSize: '0.85rem', color: 'var(--text-muted)' }}>To</label>
          <input type="date" value={dateToInput} onChange={(e) => setDateToInput(e.target.value)} />
        </div>
        {(dateFromInput || dateToInput) && (
          <button
            className="btn btn-secondary btn-small"
            onClick={() => {
              setDateFromInput('');
              setDateToInput('');
            }}
          >
            Clear range
          </button>
        )}
      </div>

      <div className="card card-table" style={{ marginBottom: '2rem' }}>
        {studentsLoading ? (
          <SkeletonRows count={5} />
        ) : students.length === 0 ? (
          <div className="empty-state">
            <h3>No Students Found</h3>
            {search && <p>No students match "{search}".</p>}
          </div>
        ) : (
          <>
            <DataTable
              rows={students}
              columns={columns}
              rowKey={(s) => s.studentId}
              expandedRowKey={expandedId}
              renderExpandedRow={(s) => (
                <div style={{ padding: '16px 20px', background: 'var(--bg-subtle, rgba(127,127,127,0.06))' }}>
                  <h4 style={{ margin: '0 0 12px 0' }}>
                    {s.name} ({s.rollNumber}) — {s.batchName} — Session Detail
                  </h4>
                  {sessionLoading[s.studentId] && !sessionRows[s.studentId] ? (
                    <SkeletonRows count={3} />
                  ) : !sessionRows[s.studentId] || sessionRows[s.studentId].length === 0 ? (
                    <p style={{ color: 'var(--text-muted)', margin: 0 }}>No sessions in the selected date range.</p>
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
                          onClick={() => loadSessionsFor(s, false)}
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

      <h2 style={{ fontSize: '1.3rem', fontWeight: 700, marginBottom: '0.75rem' }}>Bulk Lookup by Roll Number</h2>
      <div className="card" style={{ padding: 20, marginBottom: '1.5rem', display: 'flex', flexDirection: 'column', gap: 16 }}>
        <div>
          <label style={{ fontWeight: 600, fontSize: '0.9rem' }}>Roll Numbers (comma or newline separated)</label>
          <textarea
            value={rollNumbersText}
            onChange={(e) => {
              setRollNumbersText(e.target.value);
              if (e.target.value) setSelectedFile(null);
            }}
            placeholder="e.g. 21B91A0501, 21B91A0502, 21B91A0503"
            rows={3}
            style={{ width: '100%', marginTop: 6, borderRadius: 8, border: '1px solid var(--border-color, #ddd)', padding: '10px 12px', fontFamily: 'inherit' }}
          />
        </div>

        <div style={{ textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.85rem' }}>— or —</div>

        <div
          className={`file-drop-zone ${isDragging ? 'dragging' : ''} ${selectedFile ? 'has-file' : ''}`}
          onDragOver={(e) => {
            e.preventDefault();
            setIsDragging(true);
          }}
          onDragLeave={() => setIsDragging(false)}
          onDrop={handleDrop}
          onClick={() => fileInputRef.current?.click()}
        >
          <input
            type="file"
            ref={fileInputRef}
            onChange={(e) => {
              const file = e.target.files?.[0];
              if (file) validateAndSetFile(file);
            }}
            accept=".csv, application/vnd.openxmlformats-officedocument.spreadsheetml.sheet, application/vnd.ms-excel"
            style={{ display: 'none' }}
          />
          {selectedFile ? (
            <div className="file-selected-info">
              <FileSpreadsheet size={28} className="text-primary" />
              <span className="file-name">{selectedFile.name}</span>
              <p className="click-to-change">Click to change file</p>
            </div>
          ) : (
            <>
              <Upload size={28} className="text-muted" />
              <p>Upload a roster/roll-number file (any column position)</p>
              <span className="text-muted" style={{ fontSize: '0.8rem' }}>Supports .csv, .xlsx, .xls, .ods</span>
            </>
          )}
        </div>

        <button className="btn btn-primary" onClick={handleBulkSearch} disabled={searching} style={{ alignSelf: 'flex-start' }}>
          <Search size={16} style={{ marginRight: 6 }} />
          {searching ? 'Searching…' : 'Search'}
        </button>
      </div>

      {result && (
        <div className="card" style={{ padding: 20 }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 16 }}>
            <h3 style={{ margin: 0 }}>
              {result.aggregates.length} of {result.aggregates.length + result.notFound.length} roll number(s) found
            </h3>
            <button className="btn btn-secondary" onClick={handleBulkExport} disabled={exporting || result.matches.length === 0}>
              <Download size={14} style={{ marginRight: 6 }} />
              {exporting ? 'Exporting…' : 'Export Results'}
            </button>
          </div>

          {result.aggregates.length === 0 && result.notFound.length === 0 ? (
            <div className="empty-state">Enter roll numbers above and search.</div>
          ) : (
            <>
              {result.aggregates.map((agg) => {
                const rows = result.matches.filter((m) => m.rollNumberQueried.toUpperCase() === agg.rollNumber);
                return (
                  <div key={agg.rollNumber} className="card" style={{ marginBottom: 12, padding: 14 }}>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', flexWrap: 'wrap', gap: 8 }}>
                      <strong>{agg.rollNumber}</strong>
                      <span style={{ fontWeight: 700, color: attendanceColor(agg.percentage) }}>
                        {agg.percentage.toFixed(1)}% overall ({agg.totalPresent}/{agg.totalSessions}) across {agg.matchedBatchCount} batch
                        {agg.matchedBatchCount === 1 ? '' : 'es'}
                      </span>
                    </div>
                    <table className="table" style={{ width: '100%', marginTop: 10 }}>
                      <thead>
                        <tr>
                          <th style={{ textAlign: 'left' }}>Name</th>
                          <th style={{ textAlign: 'left' }}>Batch</th>
                          <th style={{ textAlign: 'left' }}>Email</th>
                          <th style={{ textAlign: 'left' }}>Attendance</th>
                          <th />
                        </tr>
                      </thead>
                      <tbody>
                        {rows.map((m) => (
                          <tr key={`${m.batchId}-${m.studentId}`}>
                            <td>{m.name}</td>
                            <td>{m.batchName}</td>
                            <td>{m.email || '—'}</td>
                            <td>
                              <span className="percent-pill" style={{ color: attendanceColor(m.percentage), background: attendanceColorBg(m.percentage) }}>
                                {m.percentage.toFixed(1)}% <small>({m.present}/{m.totalSessions})</small>
                              </span>
                            </td>
                            <td>
                              <button className="btn btn-secondary btn-small" onClick={() => navigate(`/batches/${m.batchId}`)} title="Open batch">
                                <ExternalLink size={12} />
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                );
              })}

              {result.notFound.length > 0 && (
                <div className="card" style={{ padding: 14, marginTop: 12 }}>
                  <strong>Not Found ({result.notFound.length})</strong>
                  <p style={{ color: 'var(--text-muted)', marginTop: 8 }}>{result.notFound.join(', ')}</p>
                </div>
              )}
            </>
          )}
        </div>
      )}
    </div>
  );
};

export default StudentLookup;
