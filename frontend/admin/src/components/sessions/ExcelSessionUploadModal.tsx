import { useEffect, useRef, useState } from 'react';
import type { DragEvent } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import {
  AlertTriangle, CheckCircle2, ChevronLeft, FileSpreadsheet, Loader2, Upload, X, XCircle,
} from 'lucide-react';
import MultiSelect from '../ui/MultiSelect';
import InlineMentorCreate from './InlineMentorCreate';
import type { Mentor } from '../../pages/Sessions';

interface ParsedMentor { email: string; matchedAdminId: string | null; matchedName: string | null; }
interface ParsedStudent { name: string; rollNumber: string; email?: string; round?: string; }
interface ParsedGroup {
  groupIndex: number;
  track: string;
  classLabel: string;
  sessionTimeRaw: string;
  startTime: string;
  endTime: string;
  durationMinutes: number;
  collegeName: string;
  mentors: ParsedMentor[];
  unmatchedMentorEmails: string[];
  mentorEmailConflict: boolean;
  studentCount: number;
  students: ParsedStudent[];
  issues: string[];
}
interface ParseResponse {
  sourceFilename?: string;
  sessionDate: string;
  totalRows: number;
  totalGroups: number;
  parseErrors: string[];
  groups: ParsedGroup[];
}

interface GroupState {
  groupIndex: number;
  include: boolean;
  track: string;
  classLabel: string;
  sessionTimeRaw: string;
  startTime: string;
  durationMinutes: number;
  collegeName: string;
  description: string;
  assignedAdminIds: string[];
  /** Every assigned_mentor email seen for this group (matched + unmatched) — echoed back to the commit endpoint for the excel_batches audit trail. */
  rawMentorEmails: string[];
  unmatchedMentorEmails: string[];
  mentorEmailConflict: boolean;
  issues: string[];
  studentCount: number;
  students: ParsedStudent[];
  status: 'pending' | 'creating' | 'created' | 'failed';
  resultError?: string;
  resultSessionId?: string;
}

interface CommitResult {
  groupIndex: number;
  success: boolean;
  sessionId?: string;
  error?: string;
}
interface CommitResponse {
  results: CommitResult[];
  createdCount: number;
  failedCount: number;
}

interface ExcelSessionUploadModalProps {
  open: boolean;
  onClose: () => void;
  mentors: Mentor[];
  onMentorCreated: (mentor: Mentor) => void;
  onSessionsCreated: () => void;
}

type Stage = 'upload' | 'review';

/** Best-effort DD-MM-YYYY date guess from the uploaded filename (the master
 * template is named e.g. "ATT MASTER-TEMPLATE-08-06-2026 - AN.csv") — the
 * admin can always override it. Falls back to today. */
const parseDateFromFilename = (filename: string): string => {
  const match = filename.match(/(\d{2})-(\d{2})-(\d{4})/);
  if (match) {
    const [, dd, mm, yyyy] = match;
    const candidate = `${yyyy}-${mm}-${dd}`;
    if (!Number.isNaN(new Date(candidate).getTime())) return candidate;
  }
  return new Date().toISOString().slice(0, 10);
};

const ExcelSessionUploadModal = ({ open, onClose, mentors, onMentorCreated, onSessionsCreated }: ExcelSessionUploadModalProps) => {
  const [stage, setStage] = useState<Stage>('upload');
  const [file, setFile] = useState<File | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [sessionDate, setSessionDate] = useState('');
  const [defaultCollegeName, setDefaultCollegeName] = useState('');
  const [parsing, setParsing] = useState(false);
  const [parseErrors, setParseErrors] = useState<string[]>([]);
  const [sourceFilename, setSourceFilename] = useState<string | undefined>();

  const [groups, setGroups] = useState<GroupState[]>([]);
  const [activeTabIndex, setActiveTabIndex] = useState(0);
  const [committing, setCommitting] = useState(false);
  const [showCreateMentorFor, setShowCreateMentorFor] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    // Fresh state each time the modal is (re)opened.
    setStage('upload');
    setFile(null);
    setSessionDate('');
    setDefaultCollegeName('');
    setParseErrors([]);
    setGroups([]);
    setActiveTabIndex(0);
    setShowCreateMentorFor(null);
  }, [open]);

  if (!open) return null;

  const activeGroup = groups[activeTabIndex];

  const validateAndSetFile = (f: File) => {
    const nameLower = f.name.toLowerCase();
    if (!nameLower.endsWith('.csv') && !nameLower.endsWith('.xlsx') && !nameLower.endsWith('.xls') && !nameLower.endsWith('.ods')) {
      toast.error('Please upload a valid .csv, .xlsx, .xls, or .ods file');
      return;
    }
    if (f.size > 10 * 1024 * 1024) {
      toast.error('File size must be less than 10MB');
      return;
    }
    setFile(f);
    setSessionDate((prev) => prev || parseDateFromFilename(f.name));
  };

  const handleDragOver = (e: DragEvent) => { e.preventDefault(); setIsDragging(true); };
  const handleDragLeave = () => setIsDragging(false);
  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const f = e.dataTransfer.files[0];
    if (f) validateAndSetFile(f);
  };

  const handleParse = async () => {
    if (!file) { toast.error('Please select a file to upload'); return; }
    if (!sessionDate) { toast.error('Please select the session date'); return; }
    setParsing(true);
    try {
      const formData = new FormData();
      formData.append('file', file);
      formData.append('sessionDate', sessionDate);
      formData.append('collegeName', defaultCollegeName);
      const res = await axios.post<ParseResponse>('/api/admin/excel-sessions/parse', formData, {
        headers: { 'Content-Type': 'multipart/form-data' },
      });
      const data = res.data;
      if (data.groups.length === 0) {
        toast.error('No session groups were detected in this file');
        return;
      }
      const nextGroups: GroupState[] = data.groups.map((g) => ({
        groupIndex: g.groupIndex,
        include: true,
        track: g.track,
        classLabel: g.classLabel,
        sessionTimeRaw: g.sessionTimeRaw,
        startTime: g.startTime,
        durationMinutes: g.durationMinutes,
        collegeName: g.collegeName || defaultCollegeName,
        description: `${g.track} — ${g.classLabel}`,
        assignedAdminIds: g.mentors.map((m) => m.matchedAdminId).filter((id): id is string => !!id),
        rawMentorEmails: [...g.mentors.map((m) => m.email), ...g.unmatchedMentorEmails],
        unmatchedMentorEmails: g.unmatchedMentorEmails,
        mentorEmailConflict: g.mentorEmailConflict,
        issues: g.issues,
        studentCount: g.studentCount,
        students: g.students,
        status: 'pending',
      }));
      setGroups(nextGroups);
      setParseErrors(data.parseErrors);
      setSourceFilename(data.sourceFilename);
      setActiveTabIndex(0);
      setStage('review');
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to parse the uploaded file');
    } finally {
      setParsing(false);
    }
  };

  const updateActiveGroup = (patch: Partial<GroupState>) => {
    setGroups((prev) => prev.map((g, i) => (i === activeTabIndex ? { ...g, ...patch } : g)));
  };

  const handleMentorCreated = (mentor: Mentor) => {
    const email = showCreateMentorFor;
    onMentorCreated(mentor);
    if (email) {
      setGroups((prev) => prev.map((g) => {
        if (!g.rawMentorEmails.includes(email)) return g;
        return {
          ...g,
          unmatchedMentorEmails: g.unmatchedMentorEmails.filter((e) => e !== email),
          assignedAdminIds: g.assignedAdminIds.includes(mentor._id) ? g.assignedAdminIds : [...g.assignedAdminIds, mentor._id],
        };
      }));
    }
    setShowCreateMentorFor(null);
  };

  const validateGroup = (g: GroupState): string | null => {
    if (!g.collegeName.trim()) return `"${g.classLabel} · ${g.sessionTimeRaw}": college name is required`;
    if (g.assignedAdminIds.length === 0) return `"${g.classLabel} · ${g.sessionTimeRaw}": at least one mentor is required`;
    if (!(g.durationMinutes >= 5 && g.durationMinutes <= 480)) return `"${g.classLabel} · ${g.sessionTimeRaw}": duration must be 5-480 minutes`;
    return null;
  };

  const commitGroups = async (indices: number[]) => {
    for (const i of indices) {
      const err = validateGroup(groups[i]);
      if (err) {
        setActiveTabIndex(i);
        toast.error(err);
        return;
      }
    }
    setCommitting(true);
    setGroups((prev) => prev.map((g, i) => (indices.includes(i) ? { ...g, status: 'creating' } : g)));
    try {
      const payload = {
        sourceFilename,
        sessionDate,
        groups: indices.map((i) => {
          const g = groups[i];
          return {
            groupIndex: g.groupIndex,
            track: g.track,
            classLabel: g.classLabel,
            sessionTimeRaw: g.sessionTimeRaw,
            startsAt: new Date(`${sessionDate}T${g.startTime}`).toISOString(),
            durationMinutes: g.durationMinutes,
            collegeName: g.collegeName,
            description: g.description,
            assignedAdminIds: g.assignedAdminIds,
            rawMentorEmails: g.rawMentorEmails,
            students: g.students,
          };
        }),
      };
      const res = await axios.post<CommitResponse>('/api/admin/excel-sessions/commit', payload);
      const byGroupIndex = new Map(res.data.results.map((r) => [r.groupIndex, r]));
      setGroups((prev) => prev.map((g) => {
        const result = byGroupIndex.get(g.groupIndex);
        if (!result) return g;
        return { ...g, status: result.success ? 'created' : 'failed', resultError: result.error, resultSessionId: result.sessionId };
      }));
      if (res.data.createdCount > 0) {
        toast.success(`Created ${res.data.createdCount} session${res.data.createdCount === 1 ? '' : 's'}`);
        onSessionsCreated();
      }
      if (res.data.failedCount > 0) {
        toast.error(`${res.data.failedCount} session${res.data.failedCount === 1 ? '' : 's'} failed — see the list on the left for details`);
      }
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to create sessions');
      setGroups((prev) => prev.map((g, i) => (indices.includes(i) ? { ...g, status: 'pending' } : g)));
    } finally {
      setCommitting(false);
    }
  };

  const pendingIncludedIndices = groups
    .map((g, i) => ({ g, i }))
    .filter(({ g }) => g.include && g.status !== 'created')
    .map(({ i }) => i);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div
        className="modal-content"
        onClick={(e) => e.stopPropagation()}
        style={{ maxWidth: stage === 'review' ? '1080px' : '560px', width: '100%', transition: 'max-width .2s ease' }}
      >
        <div className="modal-header">
          <h3 style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <FileSpreadsheet size={18} />
            {stage === 'upload' ? 'Upload Roster — Create Multiple Sessions' : `Review ${groups.length} Detected Session${groups.length === 1 ? '' : 's'}`}
          </h3>
          <button className="modal-close" onClick={onClose} aria-label="Close">&times;</button>
        </div>

        {stage === 'upload' && (
          <div>
            <p style={{ fontSize: '0.85rem', color: 'var(--color-muted)', marginTop: '-0.5rem', marginBottom: '1rem' }}>
              Upload the master roster spreadsheet. Rows are grouped by track + class + session time — each group becomes its own batch and mentor-marked session.
            </p>
            <div className="form-group">
              <label>Roster File</label>
              <div
                className={`file-drop-zone ${isDragging ? 'dragging' : ''} ${file ? 'has-file' : ''}`}
                onDragOver={handleDragOver}
                onDragLeave={handleDragLeave}
                onDrop={handleDrop}
                onClick={() => fileInputRef.current?.click()}
              >
                <input
                  type="file"
                  ref={fileInputRef}
                  onChange={(e) => { const f = e.target.files?.[0]; if (f) validateAndSetFile(f); }}
                  accept=".csv, application/vnd.openxmlformats-officedocument.spreadsheetml.sheet, application/vnd.ms-excel"
                  style={{ display: 'none' }}
                />
                {file ? (
                  <div className="file-selected-info">
                    <FileSpreadsheet size={32} className="text-primary" />
                    <span className="file-name">{file.name}</span>
                    <span className="file-size">{(file.size / 1024).toFixed(1)} KB</span>
                    <p className="click-to-change">Click to change file</p>
                  </div>
                ) : (
                  <>
                    <Upload size={32} className="text-muted" />
                    <p>Drag &amp; drop the master roster here, or click to browse</p>
                    <span className="text-muted" style={{ fontSize: '0.8rem' }}>student_name, registration_number, track, class, session_time, round, assigned_mentor</span>
                  </>
                )}
              </div>
            </div>
            <div className="form-row">
              <div className="form-group">
                <label htmlFor="excel-session-date">Session Date</label>
                <input id="excel-session-date" type="date" value={sessionDate} onChange={(e) => setSessionDate(e.target.value)} required />
              </div>
              <div className="form-group">
                <label htmlFor="excel-default-college">Default College Name (optional)</label>
                <input id="excel-default-college" type="text" value={defaultCollegeName} onChange={(e) => setDefaultCollegeName(e.target.value)} placeholder="e.g., SRM Institute" />
              </div>
            </div>
            <div className="form-actions">
              <button type="button" className="btn btn-success" onClick={handleParse} disabled={parsing || !file}>
                {parsing ? 'Parsing…' : 'Parse File'}
              </button>
              <button type="button" className="btn btn-secondary" onClick={onClose}>Cancel</button>
            </div>
          </div>
        )}

        {stage === 'review' && activeGroup && (
          <div>
            <button
              type="button"
              onClick={() => setStage('upload')}
              style={{ background: 'none', border: 'none', color: 'var(--color-primary)', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 4, fontSize: '0.82rem', padding: 0, marginBottom: '0.75rem' }}
            >
              <ChevronLeft size={14} /> Back to upload
            </button>

            {parseErrors.length > 0 && (
              <div style={{ background: 'var(--color-warning-bg, #fffbeb)', border: '1px solid var(--color-warning, #f59e0b)', borderRadius: 'var(--radius-sm)', padding: '0.6rem 0.8rem', marginBottom: '0.75rem', fontSize: '0.78rem' }}>
                <strong>{parseErrors.length} row{parseErrors.length === 1 ? '' : 's'} skipped</strong> due to missing required fields.
              </div>
            )}

            <div style={{ display: 'grid', gridTemplateColumns: '260px 1fr', gap: '1rem', alignItems: 'start' }}>
              {/* ── Left: group tab list ── */}
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.4rem', maxHeight: '60vh', overflowY: 'auto', paddingRight: 4 }}>
                {groups.map((g, i) => (
                  <button
                    key={g.groupIndex}
                    type="button"
                    onClick={() => setActiveTabIndex(i)}
                    style={{
                      textAlign: 'left', padding: '0.5rem 0.6rem', borderRadius: 'var(--radius-sm)',
                      border: `1.5px solid ${i === activeTabIndex ? 'var(--color-primary)' : 'var(--color-border)'}`,
                      background: i === activeTabIndex ? 'var(--color-primary-light)' : 'transparent',
                      cursor: 'pointer', display: 'flex', gap: 8, alignItems: 'flex-start',
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={g.include}
                      onClick={(e) => e.stopPropagation()}
                      onChange={(e) => setGroups((prev) => prev.map((gg, ii) => (ii === i ? { ...gg, include: e.target.checked } : gg)))}
                      style={{ marginTop: 3 }}
                    />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontWeight: 600, fontSize: '0.8rem', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {g.classLabel} · {g.sessionTimeRaw}
                      </div>
                      <div style={{ fontSize: '0.72rem', color: 'var(--color-muted)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {g.track}
                      </div>
                      <div style={{ fontSize: '0.72rem', color: 'var(--color-muted)' }}>{g.studentCount} students</div>
                    </div>
                    {g.status === 'created' && <CheckCircle2 size={16} color="var(--color-success-txt, #16a34a)" />}
                    {g.status === 'failed' && (
                      <span title={g.resultError} style={{ display: 'flex' }}>
                        <XCircle size={16} color="var(--color-danger-txt, #dc2626)" />
                      </span>
                    )}
                    {g.status === 'creating' && <Loader2 size={16} style={{ animation: 'spin 1s linear infinite' }} />}
                  </button>
                ))}
              </div>

              {/* ── Right: active group form ── */}
              <div style={{ maxHeight: '60vh', overflowY: 'auto', paddingRight: 4 }}>
                <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: '0.75rem', flexWrap: 'wrap' }}>
                  <span className="status-pill" style={{ background: 'var(--color-primary-light)', color: 'var(--color-primary)', border: '1px solid var(--color-primary)' }}>{activeGroup.track}</span>
                  <span className="status-pill">{activeGroup.classLabel}</span>
                  <span className="status-pill">{activeGroup.sessionTimeRaw}</span>
                  {activeGroup.status === 'created' && <span style={{ color: 'var(--color-success-txt, #16a34a)', fontSize: '0.78rem', fontWeight: 600 }}>Created</span>}
                  {activeGroup.status === 'failed' && <span style={{ color: 'var(--color-danger-txt, #dc2626)', fontSize: '0.78rem', fontWeight: 600 }}>Failed: {activeGroup.resultError}</span>}
                </div>

                {activeGroup.issues.length > 0 && (
                  <div style={{ background: 'var(--color-warning-bg, #fffbeb)', border: '1px solid var(--color-warning, #f59e0b)', borderRadius: 'var(--radius-sm)', padding: '0.5rem 0.7rem', marginBottom: '0.75rem', fontSize: '0.76rem', display: 'flex', gap: 6 }}>
                    <AlertTriangle size={14} style={{ flexShrink: 0, marginTop: 1 }} />
                    <span>{activeGroup.issues.join(' ')}</span>
                  </div>
                )}

                <div className="form-group">
                  <label>Assign Mentor(s)</label>
                  <MultiSelect
                    options={mentors.map((m) => ({ value: m._id, label: m.fullName || m.username }))}
                    selected={activeGroup.assignedAdminIds}
                    onChange={(next) => updateActiveGroup({ assignedAdminIds: next })}
                    placeholder="Search mentors…"
                    emptyMessage="No mentors match your search"
                  />
                  {activeGroup.unmatchedMentorEmails.length > 0 && (
                    <div style={{ marginTop: '0.5rem', display: 'flex', flexDirection: 'column', gap: '0.4rem' }}>
                      <small style={{ color: 'var(--color-muted)' }}>Emails from the sheet with no matching account:</small>
                      {activeGroup.unmatchedMentorEmails.map((email) => (
                        <div key={email}>
                          <span
                            className="status-pill"
                            style={{ borderStyle: 'dashed', borderColor: 'var(--color-warning, #f59e0b)', color: 'var(--color-warning, #f59e0b)', cursor: 'pointer' }}
                            onClick={() => setShowCreateMentorFor(showCreateMentorFor === email ? null : email)}
                          >
                            {email} — + Create account
                          </span>
                          {showCreateMentorFor === email && (
                            <InlineMentorCreate email={email} onCreated={handleMentorCreated} onCancel={() => setShowCreateMentorFor(null)} />
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </div>

                <div className="form-row">
                  <div className="form-group">
                    <label>College Name</label>
                    <input type="text" value={activeGroup.collegeName} onChange={(e) => updateActiveGroup({ collegeName: e.target.value })} required />
                  </div>
                  <div className="form-group">
                    <label>Duration (minutes)</label>
                    <input type="number" min={5} max={480} value={activeGroup.durationMinutes} onChange={(e) => updateActiveGroup({ durationMinutes: parseInt(e.target.value) || 0 })} required />
                  </div>
                </div>
                <div className="form-group">
                  <label>Description</label>
                  <input type="text" value={activeGroup.description} onChange={(e) => updateActiveGroup({ description: e.target.value })} />
                </div>

                <div className="form-group">
                  <label>Students ({activeGroup.students.length})</label>
                  <div style={{ maxHeight: 200, overflowY: 'auto', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)' }}>
                    <table className="table" style={{ minWidth: 0 }}>
                      <thead>
                        <tr><th style={{ textAlign: 'left' }}>Roll No.</th><th style={{ textAlign: 'left' }}>Name</th><th style={{ textAlign: 'left' }}>Round</th></tr>
                      </thead>
                      <tbody>
                        {activeGroup.students.slice(0, 50).map((s) => (
                          <tr key={s.rollNumber}><td>{s.rollNumber}</td><td>{s.name}</td><td>{s.round || '—'}</td></tr>
                        ))}
                      </tbody>
                    </table>
                    {activeGroup.students.length > 50 && (
                      <div style={{ padding: '0.4rem 0.7rem', fontSize: '0.75rem', color: 'var(--color-muted)' }}>
                        +{activeGroup.students.length - 50} more
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>

            <div className="form-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => commitGroups([activeTabIndex])}
                disabled={committing || activeGroup.status === 'created'}
              >
                {activeGroup.status === 'created' ? 'Created' : 'Create This Session'}
              </button>
              <button
                type="button"
                className="btn btn-success"
                onClick={() => commitGroups(pendingIncludedIndices)}
                disabled={committing || pendingIncludedIndices.length === 0}
              >
                {committing ? 'Creating…' : `Create Selected (${pendingIncludedIndices.length})`}
              </button>
              <button type="button" className="btn btn-secondary" onClick={onClose}>
                <X size={14} style={{ marginRight: 4, verticalAlign: -2 }} /> Close
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default ExcelSessionUploadModal;
