import { useEffect, useState, useCallback, useRef, useMemo } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { Link } from 'react-router';
import { toast } from 'react-toastify';
import { ClipboardList, Sparkles, Link as LinkIcon, Pencil, AlertTriangle, FileSpreadsheet, Download, RefreshCw, Plus, X, Pause, Play, Trash2, ChevronDown, ChevronUp } from 'lucide-react';
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
import SessionStatsOverview from '../components/sessions/SessionStatsOverview';
import type { SessionRecordsOverview } from '../components/sessions/SessionStatsOverview';
import SessionRecordFilters, { emptyRecordFilters } from '../components/sessions/SessionRecordFilters';
import type { SessionRecordFiltersState } from '../components/sessions/SessionRecordFilters';
import InfiniteScrollSentinel from '../components/ui/InfiniteScrollSentinel';
import { downloadBlob, filenameFromContentDisposition } from '../utils/downloadBlob';

const PAGE_SIZE = 30;

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
  track?: string;
  sessionTimeRaw?: string;
  markingStatus?: 'not_started' | 'partial' | 'complete' | 'no_roster';
  rosterSize?: number;
  markedCount?: number;
  attendancePercent?: number;
  batchId?: string;
  excelBatchId?: string;
  recurringRuleId?: string;
}

interface RecurringRule {
  _id: string;
  locationId: string;
  batchId?: string;
  description?: string;
  durationMinutes: number;
  runTimesLocal: string[];
  timezone: string;
  daysOfWeek: number[];
  isActive: boolean;
  lockedShortCode?: string;
  nextRunAt: string;
  lastGeneratedAt?: string;
  lastGeneratedSessionId?: string;
  consecutiveFailures: number;
  lastError?: string;
  createdAt: string;
}

const DAY_LABELS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

const COMMON_TIMEZONES = (() => {
  try {
    // Modern browsers only — falls back to a small curated list otherwise.
    const values = (Intl as unknown as { supportedValuesOf?: (k: string) => string[] }).supportedValuesOf?.('timeZone');
    if (values && values.length > 0) return values;
  } catch { /* fall through */ }
  return ['Asia/Kolkata', 'Asia/Dubai', 'Asia/Singapore', 'Europe/London', 'America/New_York', 'America/Los_Angeles', 'UTC'];
})();

const browserTimezone = (() => {
  try { return Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC'; }
  catch { return 'UTC'; }
})();

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

const getInitialRecordFilters = (): SessionRecordFiltersState => {
  try {
    const saved = sessionStorage.getItem('sessionRecordFilters');
    return saved ? { ...emptyRecordFilters, ...JSON.parse(saved) } : emptyRecordFilters;
  } catch {
    return emptyRecordFilters;
  }
};

const buildRecordFilterQueryParams = (filters: SessionRecordFiltersState): Record<string, string> => {
  const params: Record<string, string> = {};
  if (filters.batchIds.length) params.batchIds = filters.batchIds.join(',');
  if (filters.classLabels.length) params.classLabels = filters.classLabels.join(',');
  if (filters.tracks.length) params.tracks = filters.tracks.join(',');
  if (filters.timeSlots.length) params.timeSlots = filters.timeSlots.join(',');
  if (filters.markingStatus) params.markingStatus = filters.markingStatus;
  if (filters.search.trim()) params.search = filters.search.trim();
  return params;
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
    // Cron-job (recurring) fields — only used/shown when recurrence === 'cron'.
    recurrence: 'once' as 'once' | 'cron',
    runTimesLocal: ['09:00'] as string[],
    daysOfWeek: [0, 1, 2, 3, 4, 5, 6] as number[],
    timezone: browserTimezone,
  };
  const [formData, setFormData] = useState(emptyFormData);
  const [activeShortLinks, setActiveShortLinks] = useState<ShortLink[]>([]);
  const [recurringRules, setRecurringRules] = useState<RecurringRule[]>([]);
  const [rulesExpanded, setRulesExpanded] = useState(false);
  const [ruleDeleteId, setRuleDeleteId] = useState<string | null>(null);
  const [ruleBusyId, setRuleBusyId] = useState<string | null>(null);
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
  const [recordFilters, setRecordFilters] = useState<SessionRecordFiltersState>(getInitialRecordFilters());
  const [allSessions, setAllSessions] = useState<Session[]>([]);
  const [allBatches, setAllBatches] = useState<Batch[]>([]);
  const [overview, setOverview] = useState<SessionRecordsOverview | null>(null);
  const [exportingFiltered, setExportingFiltered] = useState(false);
  const [sessionsHasMore, setSessionsHasMore] = useState(true);
  const [loadingMoreSessions, setLoadingMoreSessions] = useState(false);
  const sessionsOffsetRef = useRef(0);
  const abortRef = useRef<AbortController | null>(null);
  const filtersRef = useRef<{ locationId: string; date: string }>(getInitialFilters());
  const recordFiltersRef = useRef<SessionRecordFiltersState>(recordFilters);
  recordFiltersRef.current = recordFilters;

  const buildFilteredParams = () => {
    const filters = filtersRef.current;
    const baseParams: Record<string, string> = {};
    if (filters.locationId) baseParams.locationId = filters.locationId;
    if (filters.date) baseParams.date = filters.date;
    const recordParams = buildRecordFilterQueryParams(recordFiltersRef.current);
    return { baseParams, filteredParams: { ...baseParams, ...recordParams } };
  };

  // Fetches the current filtered window from the top. `reset=true` (mount,
  // filter changes) always starts back at one page; `reset=false` (the 60s
  // poll) re-fetches however many rows are already loaded instead, so a
  // background refresh doesn't visibly collapse a list the user has scrolled
  // further into.
  const fetchData = useCallback(async (reset = true) => {
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    try {
      const { baseParams, filteredParams } = buildFilteredParams();
      const sessionsLimit = reset ? PAGE_SIZE : Math.max(PAGE_SIZE, sessionsOffsetRef.current);

      const [sessionsRes, allSessionsRes, locationsRes, shortLinksRes, batchesRes, mentorsRes, overviewRes, rulesRes] =
        await Promise.all([
          axios.get<Session[]>('/api/admin/sessions', {
            params: { ...filteredParams, limit: sessionsLimit, offset: 0 },
            signal: abortRef.current.signal,
          }),
          // Unfiltered — feeds the record-filter bar's cascading class/track/
          // time-slot option lists, which must reflect everything available,
          // not just what the currently active filters already narrowed to.
          axios.get<Session[]>('/api/admin/sessions', { params: baseParams, signal: abortRef.current.signal }),
          axios.get<Location[]>('/api/admin/locations', { signal: abortRef.current.signal }),
          axios.get<{ shortLinks: ShortLink[] }>('/api/admin/shortlinks', { signal: abortRef.current.signal }),
          axios.get<Batch[]>('/api/admin/batches', { signal: abortRef.current.signal }),
          axios.get<Mentor[]>('/api/admin/users', { signal: abortRef.current.signal }),
          axios.get<SessionRecordsOverview>('/api/admin/sessions/stats-overview', {
            params: filteredParams,
            signal: abortRef.current.signal,
          }),
          axios.get<RecurringRule[]>('/api/admin/recurring-rules', { signal: abortRef.current.signal }),
        ]);
      setSessions(sessionsRes.data);
      sessionsOffsetRef.current = sessionsRes.data.length;
      setSessionsHasMore(sessionsRes.data.length === sessionsLimit);
      setAllSessions(allSessionsRes.data);
      setLocations(locationsRes.data);
      setActiveShortLinks((shortLinksRes.data.shortLinks ?? []).filter((l) => l.isActive || !l.sessionId));
      // Session-batches (Excel-upload bulk creation) are 1:1 with the
      // session that generated them — they don't belong in the manual
      // "pick a batch" dropdown below.
      setBatches(batchesRes.data.filter((b) => b.type !== 'session'));
      setAllBatches(batchesRes.data);
      setMentors(mentorsRes.data.filter((m) => m.role === 'admin' && m.isActive));
      setOverview(overviewRes.data);
      setRecurringRules(rulesRes.data);
    } catch (error) {
      if ((error as { name?: string }).name !== 'CanceledError') toast.error('Failed to fetch data');
    } finally { setLoading(false); }
  }, []);

  const fetchRecurringRules = async () => {
    try {
      const res = await axios.get<RecurringRule[]>('/api/admin/recurring-rules');
      setRecurringRules(res.data);
    } catch { toast.error('Failed to refresh recurring rules'); }
  };

  // Scroll-triggered: fetches and appends the next page of sessions without
  // touching anything else (locations/batches/mentors/overview stay as-is).
  const fetchMoreSessions = async () => {
    if (!sessionsHasMore || loadingMoreSessions) return;
    setLoadingMoreSessions(true);
    try {
      const { filteredParams } = buildFilteredParams();
      const res = await axios.get<Session[]>('/api/admin/sessions', {
        params: { ...filteredParams, limit: PAGE_SIZE, offset: sessionsOffsetRef.current },
      });
      setSessions((prev) => [...prev, ...res.data]);
      sessionsOffsetRef.current += res.data.length;
      setSessionsHasMore(res.data.length === PAGE_SIZE);
    } catch {
      toast.error('Failed to load more sessions');
    } finally {
      setLoadingMoreSessions(false);
    }
  };

  const handleRecordFiltersChange = (next: SessionRecordFiltersState) => {
    setRecordFilters(next);
    try { sessionStorage.setItem('sessionRecordFilters', JSON.stringify(next)); } catch { /* ignore */ }
    recordFiltersRef.current = next;
    fetchData(true);
  };

  const handleExportFiltered = async () => {
    setExportingFiltered(true);
    try {
      const filters = filtersRef.current;
      const body = {
        locationId: filters.locationId || undefined,
        date: filters.date || undefined,
        batchIds: recordFilters.batchIds,
        classLabels: recordFilters.classLabels,
        tracks: recordFilters.tracks,
        timeSlots: recordFilters.timeSlots,
        markingStatus: recordFilters.markingStatus || undefined,
        search: recordFilters.search.trim() || undefined,
      };
      const res = await axios.post('/api/admin/sessions/export-filtered', body, { responseType: 'blob' });
      const filename = filenameFromContentDisposition(res.headers['content-disposition'], 'Attendance_Export_Filtered.xlsx');
      downloadBlob(new Blob([res.data]), filename);
      toast.success(`Exported ${overview?.filteredRecords ?? 0} matching session${overview?.filteredRecords === 1 ? '' : 's'}`);
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to export filtered sessions');
    } finally {
      setExportingFiltered(false);
    }
  };

  useEffect(() => {
    fetchData(true);
    // A background refresh, not a reset — see fetchData's `reset` param.
    const interval = setInterval(() => fetchData(false), 60000);
    return () => { clearInterval(interval); abortRef.current?.abort(); };
  }, [fetchData]);

  const handleFilterChange = useCallback((newFilters: { locationId: string; date: string }) => {
    filtersRef.current = newFilters;
    fetchData(true);
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
      const isCron = !isExam && formData.recurrence === 'cron';
      if (isExam) {
        if (!formData.batchId) { toast.error('A batch is required for an exam session'); return; }
        if (formData.assignedAdminIds.length === 0) { toast.error('Assign at least one mentor to this exam session'); return; }
        if (!formData.collegeName.trim()) { toast.error('College name is required for an exam session'); return; }
        if (!formData.startsDate || !formData.startsTime) { toast.error('Start date and time are required for an exam session'); return; }
      } else if (!formData.locationId) {
        toast.error('A location is required for a self check-in session');
        return;
      }
      if (isCron) {
        if (formData.runTimesLocal.length === 0) { toast.error('Add at least one time of day'); return; }
        if (formData.daysOfWeek.length === 0) { toast.error('Select at least one day of the week'); return; }
      }
      if (formData.shortlinkMode === 'existing' && !formData.existingShortCode) {
        toast.error('Please select an existing short link, or switch to a different mode.');
        return;
      }

      // Check for reassigning existing short link confirmation — cron rules
      // always reject an already-locked code server-side, so this
      // reassignment flow only ever applies to the one-off path.
      if (!isCron && formData.shortlinkMode === 'existing' && formData.existingShortCode && !forceReassign) {
        const selectedLink = activeShortLinks.find(l => l.shortCode === formData.existingShortCode);
        if (selectedLink && selectedLink.sessionId) {
          setReassignConfirm({ open: true, shortCode: formData.existingShortCode });
          return;
        }
      }

      if (isCron) {
        await axios.post('/api/admin/recurring-rules', {
          locationId: formData.locationId,
          batchId: formData.batchId || undefined,
          description: formData.description,
          durationMinutes: duration,
          runTimesLocal: formData.runTimesLocal,
          timezone: formData.timezone,
          daysOfWeek: formData.daysOfWeek,
          shortlinkMode: formData.shortlinkMode,
          customShortCode: formData.customShortCode.trim() || undefined,
          existingShortCode: formData.existingShortCode || undefined,
        });
        toast.success('Recurring session rule created — it will start generating sessions at its scheduled times.');
        setShowModal(false);
        setFormData(emptyFormData);
        setRulesExpanded(true);
        fetchData();
        return;
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

  const handlePauseRule = async (id: string) => {
    setRuleBusyId(id);
    try {
      await axios.post(`/api/admin/recurring-rules/${id}/pause`);
      toast.success('Rule paused — its locked link (if any) is now free to reuse');
      fetchRecurringRules();
    } catch { toast.error('Failed to pause rule'); }
    finally { setRuleBusyId(null); }
  };

  const handleResumeRule = async (id: string) => {
    setRuleBusyId(id);
    try {
      await axios.post(`/api/admin/recurring-rules/${id}/resume`);
      toast.success('Rule resumed');
      fetchRecurringRules();
    } catch { toast.error('Failed to resume rule'); }
    finally { setRuleBusyId(null); }
  };

  const handleDeleteRule = async () => {
    if (!ruleDeleteId) return;
    setRuleBusyId(ruleDeleteId);
    try {
      await axios.delete(`/api/admin/recurring-rules/${ruleDeleteId}`);
      toast.success('Recurring rule deleted');
      setRuleDeleteId(null);
      fetchRecurringRules();
    } catch { toast.error('Failed to delete rule'); }
    finally { setRuleBusyId(null); }
  };

  const toggleFormDay = (day: number) => {
    setFormData((prev) => ({
      ...prev,
      daysOfWeek: prev.daysOfWeek.includes(day)
        ? prev.daysOfWeek.filter((d) => d !== day)
        : [...prev.daysOfWeek, day].sort((a, b) => a - b),
    }));
  };

  const updateRunTime = (index: number, value: string) => {
    setFormData((prev) => ({
      ...prev,
      runTimesLocal: prev.runTimesLocal.map((t, i) => (i === index ? value : t)),
    }));
  };

  const addRunTime = () => {
    setFormData((prev) => ({ ...prev, runTimesLocal: [...prev.runTimesLocal, '09:00'] }));
  };

  const removeRunTime = (index: number) => {
    setFormData((prev) => ({
      ...prev,
      runTimesLocal: prev.runTimesLocal.length > 1 ? prev.runTimesLocal.filter((_, i) => i !== index) : prev.runTimesLocal,
    }));
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
    { key: 'location', label: 'Location',   width: '12%', render: (s) => (
      <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6 }}>
        {getLocationName(s)}
        {s.recurringRuleId && (
          <span title="Auto-generated by a recurring rule" style={{ display: 'inline-flex' }}>
            <RefreshCw size={12} color="var(--color-primary, #4f46e5)" />
          </span>
        )}
      </span>
    ) },
    { key: 'college',  label: 'College',    width: '12%', render: (s) => s.collegeName || <span style={{ color: 'var(--color-muted)' }}>—</span> },
    { key: 'mentor',   label: 'Mentor',     width: '11%', render: (s) => (s.assignedAdminNames && s.assignedAdminNames.length > 0) ? s.assignedAdminNames.join(', ') : <span style={{ color: 'var(--color-muted)' }}>—</span> },
    { key: 'status',   label: 'Status',     width: '8%', render: (s) => { const st = getStatus(s); return <Badge tone={st.tone}>{st.label}</Badge>; }},
    { key: 'starts',   label: 'Starts At',  width: '12%', render: (s) => s.startsAt ? new Date(s.startsAt).toLocaleString() : '—' },
    { key: 'students', label: 'Students',   width: '7%', align: 'center', render: (s) => s.attendanceCount },
    { key: 'attendance', label: 'Attendance %', width: '9%', align: 'center', render: (s) => {
      if (s.attendancePercent == null) return <span style={{ color: 'var(--color-muted)' }}>—</span>;
      if (s.markingStatus === 'not_started') return <Badge tone="warning">Not started</Badge>;
      return <span>{Math.round(s.attendancePercent)}%{s.markingStatus === 'partial' ? ' (partial)' : ''}</span>;
    }},
    { key: 'actions',  label: 'Actions',    width: '20%', render: (s) => (
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
          <button className="btn btn-secondary" onClick={() => setRulesExpanded((v) => !v)}>
            <RefreshCw size={15} style={{ marginRight: 6, verticalAlign: -2 }} />
            Recurring Rules {recurringRules.length > 0 ? `(${recurringRules.length})` : ''}
            {rulesExpanded ? <ChevronUp size={14} style={{ marginLeft: 6, verticalAlign: -2 }} /> : <ChevronDown size={14} style={{ marginLeft: 6, verticalAlign: -2 }} />}
          </button>
        </PageHeader>
        <SessionFilters locations={locations} onFilterChange={handleFilterChange} />
      </div>

      {locations.length === 0 && (
        <div className="card"><p>No locations found. <Link to="/locations">Create one</Link> if you need a self check-in (normal) session — exam sessions don't require a location.</p></div>
      )}

      {rulesExpanded && (
        <div className="card card-table mb-4">
          {recurringRules.length === 0 ? (
            <div style={{ padding: '1.25rem', color: 'var(--color-muted)', fontSize: '0.88rem' }}>
              No recurring rules yet. Create a session and choose "Cron job (recurring)" to set one up.
            </div>
          ) : (
            <DataTable
              rowKey={(r) => r._id}
              rows={recurringRules}
              columns={[
                { key: 'location', label: 'Location', width: '14%', render: (r) => locations.find((l) => l._id === r.locationId)?.name || 'Unknown' },
                { key: 'times', label: 'Times', width: '16%', render: (r) => r.runTimesLocal.join(', ') },
                { key: 'days', label: 'Days', width: '16%', render: (r) => r.daysOfWeek.length === 7 ? 'Every day' : r.daysOfWeek.map((d) => DAY_LABELS[d]).join(', ') },
                { key: 'timezone', label: 'Timezone', width: '13%', render: (r) => r.timezone },
                { key: 'link', label: 'Locked Link', width: '11%', render: (r) => r.lockedShortCode ? `/s/${r.lockedShortCode}` : <span style={{ color: 'var(--color-muted)' }}>—</span> },
                { key: 'next', label: 'Next Run', width: '13%', render: (r) => r.isActive ? new Date(r.nextRunAt).toLocaleString() : <span style={{ color: 'var(--color-muted)' }}>Paused</span> },
                { key: 'status', label: 'Status', width: '8%', render: (r) => {
                  if (!r.isActive) return <Badge tone="neutral">Paused</Badge>;
                  if (r.consecutiveFailures > 0) return <Badge tone="warning">Attention</Badge>;
                  return <Badge tone="success">Active</Badge>;
                } },
                { key: 'actions', label: 'Actions', width: '9%', render: (r) => (
                  <div className="actions-cell">
                    {r.isActive ? (
                      <Button variant="secondary" size="sm" disabled={ruleBusyId === r._id} onClick={() => handlePauseRule(r._id)}>
                        <Pause size={13} style={{ marginRight: 4, verticalAlign: -2 }} />Pause
                      </Button>
                    ) : (
                      <Button variant="secondary" size="sm" disabled={ruleBusyId === r._id} onClick={() => handleResumeRule(r._id)}>
                        <Play size={13} style={{ marginRight: 4, verticalAlign: -2 }} />Resume
                      </Button>
                    )}
                    <Button variant="delete" size="sm" disabled={ruleBusyId === r._id} onClick={() => setRuleDeleteId(r._id)}>
                      <Trash2 size={13} style={{ marginRight: 4, verticalAlign: -2 }} />Delete
                    </Button>
                  </div>
                ) },
              ]}
            />
          )}
        </div>
      )}

      <SessionStatsOverview overview={overview} />

      <SessionRecordFilters
        allSessions={allSessions}
        batches={allBatches}
        filters={recordFilters}
        onChange={handleRecordFiltersChange}
        onExportFiltered={handleExportFiltered}
        exporting={exportingFiltered}
        exportCount={overview?.filteredRecords ?? 0}
      />

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
          {sessionsHasMore && (
            <>
              <InfiniteScrollSentinel onIntersect={fetchMoreSessions} disabled={loadingMoreSessions} />
              {loadingMoreSessions && <div className="load-more-indicator">Loading more…</div>}
            </>
          )}
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

          {formData.sessionType === 'normal' && (
            <div className="form-group">
              <label>Cadence</label>
              <div style={{ display: 'flex', gap: '0.4rem', marginTop: '0.375rem' }}>
                {(['once', 'cron'] as const).map((mode) => (
                  <label key={mode} style={{
                    flex: 1, textAlign: 'center', padding: '0.45rem 0.25rem',
                    borderRadius: '6px', cursor: 'pointer', fontSize: '0.78rem', fontWeight: 500,
                    border: `1.5px solid ${
                      formData.recurrence === mode ? 'var(--color-primary, #4f46e5)' : 'var(--color-border, #e5e7eb)'
                    }`,
                    background: formData.recurrence === mode ? 'var(--color-primary-subtle, #eef2ff)' : 'transparent',
                    color: formData.recurrence === mode ? 'var(--color-primary, #4f46e5)' : 'var(--color-muted)',
                    transition: 'all 0.15s',
                    userSelect: 'none',
                  }}>
                    <input type="radio" name="recurrence" value={mode}
                      checked={formData.recurrence === mode}
                      onChange={() => setFormData({ ...formData, recurrence: mode })}
                      style={{ display: 'none' }} />
                    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '6px' }}>
                      {mode === 'cron' && <RefreshCw size={14} />}
                      <span>{mode === 'once' ? 'One-time' : 'Cron job (recurring)'}</span>
                    </div>
                  </label>
                ))}
              </div>
              {formData.recurrence === 'cron' && (
                <small style={{ color: 'var(--color-muted)' }}>
                  A new session for this location is created automatically at each time below, on the selected days.
                </small>
              )}
            </div>
          )}

          {formData.sessionType === 'normal' && formData.recurrence === 'cron' && (
            <>
              <div className="form-group">
                <label>Time(s) of day</label>
                {formData.runTimesLocal.map((t, i) => (
                  <div key={i} style={{ display: 'flex', gap: '0.4rem', alignItems: 'center', marginBottom: '0.4rem' }}>
                    <input type="time" value={t} onChange={(e) => updateRunTime(i, e.target.value)} required style={{ flex: 1 }} />
                    {formData.runTimesLocal.length > 1 && (
                      <button type="button" className="btn btn-secondary btn-small" onClick={() => removeRunTime(i)} aria-label="Remove time">
                        <X size={14} />
                      </button>
                    )}
                  </div>
                ))}
                <button type="button" className="btn btn-secondary btn-small" onClick={addRunTime}>
                  <Plus size={14} style={{ marginRight: 4, verticalAlign: -2 }} />
                  Add another time
                </button>
              </div>

              <div className="form-group">
                <label>Days of week</label>
                <div style={{ display: 'flex', gap: '0.3rem', marginTop: '0.375rem', flexWrap: 'wrap' }}>
                  {DAY_LABELS.map((label, day) => (
                    <label key={day} style={{
                      padding: '0.4rem 0.6rem', borderRadius: '6px', cursor: 'pointer', fontSize: '0.78rem', fontWeight: 500,
                      border: `1.5px solid ${formData.daysOfWeek.includes(day) ? 'var(--color-primary, #4f46e5)' : 'var(--color-border, #e5e7eb)'}`,
                      background: formData.daysOfWeek.includes(day) ? 'var(--color-primary-subtle, #eef2ff)' : 'transparent',
                      color: formData.daysOfWeek.includes(day) ? 'var(--color-primary, #4f46e5)' : 'var(--color-muted)',
                      userSelect: 'none',
                    }}>
                      <input type="checkbox" checked={formData.daysOfWeek.includes(day)} onChange={() => toggleFormDay(day)} style={{ display: 'none' }} />
                      {label}
                    </label>
                  ))}
                </div>
              </div>

              <div className="form-group">
                <label htmlFor="session-timezone">Timezone</label>
                <select id="session-timezone" value={formData.timezone} onChange={(e) => setFormData({ ...formData, timezone: e.target.value })} required>
                  {!COMMON_TIMEZONES.includes(formData.timezone) && <option value={formData.timezone}>{formData.timezone}</option>}
                  {COMMON_TIMEZONES.map((tz) => <option key={tz} value={tz}>{tz}</option>)}
                </select>
              </div>
            </>
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

          {formData.sessionType === 'normal' && formData.recurrence === 'cron' && (
            <p style={{ fontSize: '0.8rem', color: 'var(--color-muted)', marginTop: '-0.25rem', marginBottom: '0.5rem' }}>
              This link will be locked to the rule — it always redirects to whichever occurrence is currently active, and can't be picked for another one-off session while the rule is active. Pausing or deleting the rule frees it again.
            </p>
          )}

          {formData.sessionType === 'normal' && formData.recurrence === 'once' && formData.shortlinkMode === 'auto' && (
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
            <button type="submit" className="btn btn-success">
              {formData.sessionType === 'normal' && formData.recurrence === 'cron' ? 'Create Recurring Rule' : 'Create Session'}
            </button>
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

      <ConfirmModal
        isOpen={!!ruleDeleteId}
        title="Delete Recurring Rule"
        message="Are you sure you want to delete this recurring rule? It will stop generating new sessions, and its locked link (if any) will be freed for reuse. Sessions it already created are not affected."
        confirmText="Delete"
        onConfirm={handleDeleteRule}
        onCancel={() => setRuleDeleteId(null)}
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
