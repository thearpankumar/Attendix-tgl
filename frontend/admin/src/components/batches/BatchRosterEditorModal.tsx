import { useCallback, useEffect, useRef, useState } from 'react';
import type { DragEvent } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { FileSpreadsheet, Plus, Trash2, Upload } from 'lucide-react';

interface BatchRef {
  _id: string;
  name: string;
}

interface StudentFields {
  name: string;
  rollNumber: string;
  collegeName?: string | null;
  email?: string | null;
}

interface ServerStudent extends StudentFields {
  id: string;
}

interface Row {
  /** Stable React key. */
  key: string;
  /** Present once the row exists on the server. */
  id?: string;
  name: string;
  rollNumber: string;
  collegeName: string;
  email: string;
  /** Marked for deletion; applied on Save. */
  deleted?: boolean;
  /** Server-side error from the last save attempt. */
  error?: string;
}

interface UpdatedStudent { id: string; before: StudentFields; after: StudentFields }
interface RosterDiff {
  mode: SyncMode;
  added: StudentFields[];
  updated: UpdatedStudent[];
  removed: (StudentFields & { id: string })[];
  unchanged: number;
}
interface ParseResponse { students: StudentFields[]; errors: string[] }

type SyncMode = 'merge' | 'replace';
type Tab = 'edit' | 'upload';

interface Props {
  batch: BatchRef | null;
  onClose: () => void;
  /** Called after any roster change was saved, so the list can refresh counts. */
  onChanged: () => void;
}

let keyCounter = 0;
const nextKey = () => `row-${++keyCounter}`;

const toRow = (s: ServerStudent): Row => ({
  key: nextKey(),
  id: s.id,
  name: s.name,
  rollNumber: s.rollNumber,
  collegeName: s.collegeName ?? '',
  email: s.email ?? '',
});

const emptyRow = (): Row => ({ key: nextKey(), name: '', rollNumber: '', collegeName: '', email: '' });

const rollKey = (r: string) => r.trim().toUpperCase();

const errorMessage = (error: unknown, fallback: string): string => {
  const err = error as { response?: { data?: { message?: string } } };
  return err.response?.data?.message || fallback;
};

const BatchRosterEditorModal = ({ batch, onClose, onChanged }: Props) => {
  const [tab, setTab] = useState<Tab>('edit');
  const [rows, setRows] = useState<Row[]>([]);
  const [original, setOriginal] = useState<Record<string, Row>>({});
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const changedRef = useRef(false);

  // Upload state
  const [mode, setMode] = useState<SyncMode>('merge');
  const [file, setFile] = useState<File | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [busy, setBusy] = useState(false);
  const [parsed, setParsed] = useState<ParseResponse | null>(null);
  const [diff, setDiff] = useState<RosterDiff | null>(null);

  const batchId = batch?._id;

  const loadRoster = useCallback(async () => {
    if (!batchId) return;
    setLoading(true);
    try {
      const { data } = await axios.get<{ students: ServerStudent[] }>(`/api/admin/batches/${batchId}`);
      const loaded = (data.students ?? []).map(toRow);
      setRows(loaded);
      setOriginal(Object.fromEntries(loaded.map((r) => [r.id as string, r])));
    } catch (_err) {
      toast.error('Failed to load the batch roster');
      onClose();
    } finally {
      setLoading(false);
    }
  }, [batchId]);

  useEffect(() => {
    if (!batchId) return;
    setTab('edit');
    setFile(null);
    setParsed(null);
    setDiff(null);
    setMode('merge');
    changedRef.current = false;
    loadRoster();
  }, [batchId, loadRoster]);

  if (!batch) return null;

  const close = () => {
    if (changedRef.current) onChanged();
    onClose();
  };

  // ---------- Spreadsheet-style editing ----------

  const updateRow = (key: string, patch: Partial<Row>) =>
    setRows((prev) => prev.map((r) => (r.key === key ? { ...r, ...patch, error: undefined } : r)));

  const removeRow = (row: Row) => {
    if (row.id) updateRow(row.key, { deleted: true });
    else setRows((prev) => prev.filter((r) => r.key !== row.key));
  };

  const live = rows.filter((r) => !r.deleted);
  const rollCounts = new Map<string, number>();
  live.forEach((r) => {
    const k = rollKey(r.rollNumber);
    if (k) rollCounts.set(k, (rollCounts.get(k) ?? 0) + 1);
  });
  const rowProblem = (r: Row): string | undefined => {
    if (r.deleted) return undefined;
    if (!r.name.trim()) return 'Name is required';
    if (!r.rollNumber.trim()) return 'Roll number is required';
    if ((rollCounts.get(rollKey(r.rollNumber)) ?? 0) > 1) return 'Duplicate roll number in this batch';
    return undefined;
  };

  const isDirty = (r: Row) => {
    if (r.deleted) return !!r.id;
    if (!r.id) return true;
    const o = original[r.id];
    return !o || o.name !== r.name || o.rollNumber !== r.rollNumber || o.collegeName !== r.collegeName || o.email !== r.email;
  };
  const dirtyRows = rows.filter(isDirty);
  const hasProblems = live.some((r) => isDirty(r) && rowProblem(r));

  const payload = (r: Row) => ({
    name: r.name.trim(),
    rollNumber: r.rollNumber.trim(),
    collegeName: r.collegeName.trim() || null,
    email: r.email.trim() || null,
  });

  const saveChanges = async () => {
    if (!batchId || saving) return;
    setSaving(true);
    let failures = 0;
    let successes = 0;
    // Deletes first so a roll number freed by a removed row can be reused by
    // an edited or new one in the same save.
    const ordered = [...dirtyRows.filter((r) => r.deleted), ...dirtyRows.filter((r) => !r.deleted && r.id), ...dirtyRows.filter((r) => !r.deleted && !r.id)];
    for (const r of ordered) {
      try {
        if (r.deleted && r.id) {
          await axios.delete(`/api/admin/batches/${batchId}/students/${r.id}`);
          setRows((prev) => prev.filter((x) => x.key !== r.key));
        } else if (r.id) {
          await axios.patch(`/api/admin/batches/${batchId}/students/${r.id}`, payload(r));
        } else {
          const { data } = await axios.post<ServerStudent>(`/api/admin/batches/${batchId}/students`, payload(r));
          setRows((prev) => prev.map((x) => (x.key === r.key ? { ...x, id: data?.id } : x)));
        }
        successes += 1;
      } catch (error) {
        failures += 1;
        const message = errorMessage(error, 'Failed to save this row');
        setRows((prev) => prev.map((x) => (x.key === r.key ? { ...x, error: message } : x)));
      }
    }
    setSaving(false);
    if (successes > 0) changedRef.current = true;
    if (failures === 0) {
      toast.success(`Roster saved (${successes} change${successes === 1 ? '' : 's'})`);
      await loadRoster();
    } else {
      toast.error(`${failures} row${failures === 1 ? '' : 's'} could not be saved — see the highlighted rows`);
      // Re-baseline the rows that did save so only the failures stay dirty.
      try {
        const { data } = await axios.get<{ students: ServerStudent[] }>(`/api/admin/batches/${batchId}`);
        const saved = (data.students ?? []).map(toRow);
        setOriginal(Object.fromEntries(saved.map((s) => [s.id as string, s])));
      } catch (_err) {
        /* keep the stale baseline; the failed rows stay highlighted either way */
      }
    }
  };

  // ---------- Update from file ----------

  const validateAndSetFile = (f: File) => {
    const nameLower = f.name.toLowerCase();
    if (!['.csv', '.xlsx', '.xls', '.ods'].some((ext) => nameLower.endsWith(ext))) {
      toast.error('Please upload a valid .csv, .xlsx, .xls, or .ods file');
      return;
    }
    if (f.size > 10 * 1024 * 1024) {
      toast.error('File size must be less than 10MB');
      return;
    }
    setFile(f);
    setParsed(null);
    setDiff(null);
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const f = e.dataTransfer.files[0];
    if (f) validateAndSetFile(f);
  };

  const requestPreview = async (students: StudentFields[], forMode: SyncMode) => {
    const { data } = await axios.post<RosterDiff>(`/api/admin/batches/${batchId}/students/preview`, { mode: forMode, students });
    setDiff(data);
  };

  const parseAndPreview = async () => {
    if (!file) { toast.error('Please select a file to upload'); return; }
    setBusy(true);
    try {
      const form = new FormData();
      form.append('file', file);
      const { data } = await axios.post<ParseResponse>(`/api/admin/batches/${batchId}/students/parse`, form, {
        headers: { 'Content-Type': 'multipart/form-data' },
      });
      setParsed(data);
      await requestPreview(data.students, mode);
    } catch (error) {
      toast.error(errorMessage(error, 'Failed to read the uploaded file'));
    } finally {
      setBusy(false);
    }
  };

  const changeMode = async (next: SyncMode) => {
    setMode(next);
    if (!parsed) return;
    setBusy(true);
    try {
      await requestPreview(parsed.students, next);
    } catch (error) {
      toast.error(errorMessage(error, 'Failed to preview the changes'));
    } finally {
      setBusy(false);
    }
  };

  const applyUpload = async () => {
    if (!parsed || !diff) return;
    setBusy(true);
    try {
      await axios.put(`/api/admin/batches/${batchId}/students`, { mode, students: parsed.students, source: 'excel' });
      toast.success('Roster updated');
      changedRef.current = true;
      setFile(null);
      setParsed(null);
      setDiff(null);
      setTab('edit');
      await loadRoster();
    } catch (error) {
      toast.error(errorMessage(error, 'Failed to update the roster'));
    } finally {
      setBusy(false);
    }
  };

  const nothingToApply = !!diff && diff.added.length === 0 && diff.updated.length === 0 && diff.removed.length === 0;

  const cellInput = (r: Row, field: 'name' | 'rollNumber' | 'collegeName' | 'email', label: string, type = 'text') => (
    <input
      type={type}
      aria-label={`${label} for row ${rows.indexOf(r) + 1}`}
      value={r[field]}
      disabled={r.deleted || saving}
      onChange={(e) => updateRow(r.key, { [field]: e.target.value })}
      style={{ width: '100%', padding: '6px 8px', fontSize: '0.85rem', boxSizing: 'border-box' }}
    />
  );

  const diffList = (title: string, items: React.ReactNode[], tone: string) =>
    items.length > 0 && (
      <details open={items.length <= 8} style={{ marginBottom: '0.5rem' }}>
        <summary style={{ cursor: 'pointer', fontWeight: 600, color: tone }}>{title} ({items.length})</summary>
        <ul style={{ margin: '0.3rem 0 0 1rem', padding: 0, fontSize: '0.82rem', maxHeight: 160, overflowY: 'auto' }}>
          {items.map((it, i) => <li key={i}>{it}</li>)}
        </ul>
      </details>
    );

  return (
    <div className="modal-overlay" onClick={close}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()} style={{ maxWidth: '960px', width: '100%' }}>
        <div className="modal-header">
          <h3>Edit Roster — {batch.name}</h3>
          <button className="modal-close" onClick={close} aria-label="Close">&times;</button>
        </div>

        <div style={{ display: 'flex', gap: 8, marginBottom: '1rem' }} role="tablist">
          <button role="tab" aria-selected={tab === 'edit'} className={`btn btn-small ${tab === 'edit' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setTab('edit')}>
            Edit students
          </button>
          <button role="tab" aria-selected={tab === 'upload'} className={`btn btn-small ${tab === 'upload' ? 'btn-primary' : 'btn-secondary'}`} onClick={() => setTab('upload')}>
            <Upload size={13} style={{ marginRight: 4 }} /> Update from file
          </button>
        </div>

        {tab === 'edit' && (
          <div>
            <p style={{ fontSize: '0.82rem', color: 'var(--color-muted)', marginTop: 0 }}>
              Edit cells directly, add rows, or remove students, then save. Roll numbers must be unique in the batch (case-insensitive).
              Correcting a student's roll number also moves their past check-ins (and passkey) to the new roll number, so earlier sessions keep showing them present;
              this is refused if a session already has check-ins under both numbers. Changing a name never rewrites past records, and removing a student keeps their records.
            </p>
            {loading ? (
              <div className="loading">Loading roster…</div>
            ) : (
              <>
                <div style={{ maxHeight: '48vh', overflow: 'auto', border: '1px solid var(--color-border)', borderRadius: 'var(--radius-sm)' }}>
                  <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.85rem' }}>
                    <thead>
                      <tr style={{ position: 'sticky', top: 0, background: 'var(--color-surface, #fff)', textAlign: 'left' }}>
                        <th style={{ padding: '6px 8px', width: 36 }}>#</th>
                        <th style={{ padding: '6px 8px' }}>Roll number</th>
                        <th style={{ padding: '6px 8px' }}>Name</th>
                        <th style={{ padding: '6px 8px' }}>College</th>
                        <th style={{ padding: '6px 8px' }}>Email</th>
                        <th style={{ padding: '6px 8px', width: 48 }} />
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map((r, i) => {
                        const problem = isDirty(r) ? rowProblem(r) : undefined;
                        const message = r.error || problem;
                        return (
                          <tr
                            key={r.key}
                            data-testid="roster-row"
                            style={{
                              opacity: r.deleted ? 0.45 : 1,
                              textDecoration: r.deleted ? 'line-through' : undefined,
                              background: message ? 'var(--color-danger-bg, #fef2f2)' : isDirty(r) ? 'var(--color-warning-bg, #fffbeb)' : undefined,
                            }}
                          >
                            <td style={{ padding: '4px 8px', color: 'var(--color-muted)' }}>{i + 1}</td>
                            <td style={{ padding: '4px 4px' }}>{cellInput(r, 'rollNumber', 'Roll number')}</td>
                            <td style={{ padding: '4px 4px' }}>{cellInput(r, 'name', 'Name')}</td>
                            <td style={{ padding: '4px 4px' }}>{cellInput(r, 'collegeName', 'College')}</td>
                            <td style={{ padding: '4px 4px' }}>
                              {cellInput(r, 'email', 'Email', 'email')}
                              {message && <div role="alert" style={{ color: 'var(--color-danger-txt, #dc2626)', fontSize: '0.72rem' }}>{message}</div>}
                            </td>
                            <td style={{ padding: '4px 4px', textAlign: 'center' }}>
                              {r.deleted ? (
                                <button type="button" className="btn btn-secondary btn-small" onClick={() => updateRow(r.key, { deleted: false })} title="Undo remove" disabled={saving}>Undo</button>
                              ) : (
                                <button type="button" className="btn btn-danger btn-small" onClick={() => removeRow(r)} title="Remove student" aria-label={`Remove row ${i + 1}`} disabled={saving}>
                                  <Trash2 size={13} />
                                </button>
                              )}
                            </td>
                          </tr>
                        );
                      })}
                      {rows.length === 0 && (
                        <tr><td colSpan={6} style={{ padding: 16, textAlign: 'center', color: 'var(--color-muted)' }}>No students yet — add a row.</td></tr>
                      )}
                    </tbody>
                  </table>
                </div>
                <div className="form-actions" style={{ justifyContent: 'space-between', marginTop: '0.75rem' }}>
                  <button type="button" className="btn btn-secondary" onClick={() => setRows((prev) => [...prev, emptyRow()])} disabled={saving}>
                    <Plus size={14} style={{ marginRight: 4 }} /> Add row
                  </button>
                  <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                    {dirtyRows.length > 0 && <span style={{ fontSize: '0.8rem', color: 'var(--color-muted)' }}>{dirtyRows.length} unsaved change{dirtyRows.length === 1 ? '' : 's'}</span>}
                    <button type="button" className="btn btn-primary" onClick={saveChanges} disabled={saving || dirtyRows.length === 0 || hasProblems}>
                      {saving ? 'Saving…' : 'Save changes'}
                    </button>
                    <button type="button" className="btn btn-secondary" onClick={close}>Close</button>
                  </div>
                </div>
              </>
            )}
          </div>
        )}

        {tab === 'upload' && (
          <div>
            <div className="form-group">
              <label>How should the file update this batch?</label>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
                <label style={{ display: 'flex', gap: 8, alignItems: 'flex-start', fontWeight: 400 }}>
                  <input type="radio" name="roster-mode" checked={mode === 'merge'} onChange={() => changeMode('merge')} disabled={busy} />
                  <span><strong>Add &amp; update (merge)</strong> — add new students and update existing ones matched by roll number. Nobody is removed.</span>
                </label>
                <label style={{ display: 'flex', gap: 8, alignItems: 'flex-start', fontWeight: 400 }}>
                  <input type="radio" name="roster-mode" checked={mode === 'replace'} onChange={() => changeMode('replace')} disabled={busy} />
                  <span><strong>Replace roster</strong> — the batch will match the file exactly; students not in the file are removed (their past attendance is kept).</span>
                </label>
              </div>
            </div>

            <div className="form-group">
              <label>Roster File (CSV / Excel)</label>
              <div
                className={`file-drop-zone ${isDragging ? 'dragging' : ''} ${file ? 'has-file' : ''}`}
                onDragOver={(e) => { e.preventDefault(); setIsDragging(true); }}
                onDragLeave={() => setIsDragging(false)}
                onDrop={handleDrop}
                onClick={() => fileInputRef.current?.click()}
              >
                <input
                  type="file"
                  ref={fileInputRef}
                  onChange={(e) => { const f = e.target.files?.[0]; if (f) validateAndSetFile(f); }}
                  accept=".csv,.xlsx,.xls,.ods"
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
                    <p>Drag &amp; drop your file here, or click to browse</p>
                    <span className="text-muted" style={{ fontSize: '0.8rem' }}>Supports: Roll / Reg No, Student Name, Email, College (any column order)</span>
                  </>
                )}
              </div>
            </div>

            {!diff && (
              <div className="form-actions">
                <button type="button" className="btn btn-success" onClick={parseAndPreview} disabled={busy || !file}>
                  {busy ? 'Reading…' : 'Preview changes'}
                </button>
                <button type="button" className="btn btn-secondary" onClick={close}>Cancel</button>
              </div>
            )}

            {diff && parsed && (
              <div data-testid="roster-diff">
                <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: '0.6rem' }}>
                  <span className="status-pill">{diff.added.length} to add</span>
                  <span className="status-pill">{diff.updated.length} to update</span>
                  <span className="status-pill">{diff.removed.length} to remove</span>
                  <span className="status-pill">{diff.unchanged} unchanged</span>
                </div>
                {parsed.errors.length > 0 && (
                  <div style={{ background: 'var(--color-warning-bg, #fffbeb)', border: '1px solid var(--color-warning, #f59e0b)', borderRadius: 'var(--radius-sm)', padding: '0.5rem 0.7rem', marginBottom: '0.6rem', fontSize: '0.78rem' }}>
                    <strong>{parsed.errors.length} row{parsed.errors.length === 1 ? '' : 's'} skipped</strong>
                    <ul style={{ margin: '0.25rem 0 0 1rem', padding: 0, maxHeight: 90, overflowY: 'auto' }}>
                      {parsed.errors.map((e, i) => <li key={i}>{e}</li>)}
                    </ul>
                  </div>
                )}
                {diffList('Added', diff.added.map((s) => `${s.rollNumber} — ${s.name}`), 'var(--color-success-txt, #16a34a)')}
                {diffList(
                  'Updated',
                  diff.updated.map((u) => {
                    const changes: string[] = [];
                    if (u.before.name !== u.after.name) changes.push(`name "${u.before.name}" → "${u.after.name}"`);
                    if ((u.before.collegeName ?? '') !== (u.after.collegeName ?? '')) changes.push(`college "${u.before.collegeName ?? ''}" → "${u.after.collegeName ?? ''}"`);
                    if ((u.before.email ?? '') !== (u.after.email ?? '')) changes.push(`email "${u.before.email ?? ''}" → "${u.after.email ?? ''}"`);
                    return `${u.before.rollNumber}: ${changes.join(', ')}`;
                  }),
                  'var(--color-primary)',
                )}
                {diffList('Removed', diff.removed.map((s) => `${s.rollNumber} — ${s.name}`), 'var(--color-danger-txt, #dc2626)')}
                {diff.removed.length > 0 && (
                  <p style={{ fontSize: '0.78rem', color: 'var(--color-muted)' }}>
                    Removed students disappear from this batch's roster and absent lists, but their past attendance records and exports are kept.
                  </p>
                )}
                {nothingToApply && <p style={{ fontSize: '0.85rem' }}>The file matches the current roster — nothing to change.</p>}
                <div className="form-actions">
                  <button type="button" className="btn btn-primary" onClick={applyUpload} disabled={busy || nothingToApply}>
                    {busy ? 'Applying…' : mode === 'replace' ? 'Replace roster' : 'Apply changes'}
                  </button>
                  <button type="button" className="btn btn-secondary" onClick={() => { setDiff(null); setParsed(null); }} disabled={busy}>Back</button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};

export default BatchRosterEditorModal;
