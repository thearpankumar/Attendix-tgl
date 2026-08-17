import { useEffect, useState, useCallback, useMemo, useRef } from 'react';
import axios from 'axios';
import { useParams } from 'react-router';
import { toast } from 'react-toastify';
import { motion, AnimatePresence } from 'framer-motion';
import type { PanInfo } from 'framer-motion';
import { Check, X, Loader2, MapPin, Clock, Hourglass, PartyPopper, RotateCcw, Layers, List as ListIcon, Search } from 'lucide-react';

interface RosterStudent {
  studentId: string;
  rollNumber: string;
  name: string;
  collegeName?: string;
  email?: string;
  status: 'unmarked' | 'present' | 'absent';
  source: 'self_submitted' | 'manual' | null;
  markedAt: string | null;
}

interface RosterResponse {
  session: {
    _id: string;
    collegeName?: string;
    startsAt?: string;
    expiresAt: string;
    batchName?: string;
    description?: string;
  };
  students: RosterStudent[];
  summary: { total: number; marked: number; present: number; absent: number; unmarked: number };
}

const initials = (name: string) => name.split(' ').filter(Boolean).slice(0, 2).map((p) => p[0]?.toUpperCase()).join('') || '?';

const timeUntil = (iso: string, now: number) => {
  const diffMin = Math.round((new Date(iso).getTime() - now) / 60000);
  if (diffMin <= 0) return 'Starting now';
  const h = Math.floor(diffMin / 60);
  const m = diffMin % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
};

const SWIPE_THRESHOLD = 90;

type ViewMode = 'stack' | 'list';

const SessionDetail = () => {
  const { id } = useParams<{ id: string }>();
  const [data, setData] = useState<RosterResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [exitDir, setExitDir] = useState<1 | -1>(-1);
  const [pending, setPending] = useState(false);
  const [viewMode, setViewMode] = useState<ViewMode>('stack');
  const [searchQuery, setSearchQuery] = useState('');
  const [now, setNow] = useState(() => Date.now());
  const fetchedOnce = useRef(false);

  // Ticks so a mentor who opens this page before the session starts sees the
  // countdown live-update and gets the marking UI automatically once the
  // start time passes, with no manual refresh needed.
  useEffect(() => {
    const interval = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(interval);
  }, []);

  const fetchRoster = useCallback(async () => {
    if (!id) return;
    try {
      const res = await axios.get<RosterResponse>(`/api/admin/sessions/${id}/roster`);
      setData(res.data);
    } catch {
      toast.error('Failed to load the roster for this session');
    } finally {
      setLoading(false);
    }
  }, [id]);

  useEffect(() => {
    if (fetchedOnce.current) return;
    fetchedOnce.current = true;
    fetchRoster();
  }, [fetchRoster]);

  const unmarked = useMemo(() => data?.students.filter((s) => s.status === 'unmarked') ?? [], [data]);
  const markedList = useMemo(
    () => data?.students.filter((s) => s.status !== 'unmarked') ?? [],
    [data]
  );
  // List view: the whole roster, searchable, regardless of status — the
  // point of this view is being able to act on any student at any time,
  // not just the next unmarked one in sequence.
  const filteredStudents = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    const all = data?.students ?? [];
    if (!q) return all;
    return all.filter((s) => s.name.toLowerCase().includes(q) || s.rollNumber.toLowerCase().includes(q));
  }, [data, searchQuery]);

  const applyLocalStatus = (rollNumber: string, status: 'present' | 'absent' | 'unmarked') => {
    setData((prev) => {
      if (!prev) return prev;
      const students = prev.students.map((s) =>
        s.rollNumber === rollNumber ? { ...s, status, source: status === 'unmarked' ? null : 'manual' as const } : s
      );
      const present = students.filter((s) => s.status === 'present').length;
      const absent = students.filter((s) => s.status === 'absent').length;
      const marked = present + absent;
      return { ...prev, students, summary: { ...prev.summary, present, absent, marked, unmarked: prev.summary.total - marked } };
    });
  };

  const markStudent = async (student: RosterStudent, status: 'present' | 'absent') => {
    if (!id || pending) return;
    setExitDir(status === 'present' ? -1 : 1);
    setPending(true);
    applyLocalStatus(student.rollNumber, status);

    try {
      await axios.post(`/api/admin/sessions/${id}/attendance/manual`, { rollNumber: student.rollNumber, status });
      const label = status === 'present' ? 'Present' : 'Absent';
      toast.success(`${student.name} marked ${label}`, {
        autoClose: 4000,
      });
    } catch (error) {
      applyLocalStatus(student.rollNumber, 'unmarked');
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || `Failed to mark ${student.name}`);
    } finally {
      setPending(false);
    }
  };

  const undoStudent = async (student: RosterStudent) => {
    if (!id) return;
    const prevStatus = student.status;
    applyLocalStatus(student.rollNumber, 'unmarked');
    try {
      await axios.delete(`/api/admin/sessions/${id}/attendance/manual/${encodeURIComponent(student.rollNumber)}`);
      toast.info(`Undid mark for ${student.name}`);
    } catch {
      applyLocalStatus(student.rollNumber, prevStatus);
      toast.error('Failed to undo — please try again');
    }
  };

  const handleDragEnd = (student: RosterStudent, _e: unknown, info: PanInfo) => {
    if (info.offset.x < -SWIPE_THRESHOLD) markStudent(student, 'present');
    else if (info.offset.x > SWIPE_THRESHOLD) markStudent(student, 'absent');
  };

  if (loading) {
    return <div className="loading-screen" style={{ minHeight: '50vh' }}><Loader2 size={22} className="spin" /></div>;
  }

  if (!data) return null;

  const { session, summary } = data;
  const top = unmarked[0];
  const progressPct = summary.total === 0 ? 0 : Math.round((summary.marked / summary.total) * 100);
  // A session with no startsAt (a normal self-check-in session, marked
  // manually only as a fallback) has always been markable — the gate only
  // applies to sessions explicitly scheduled for the future.
  const hasStarted = !session.startsAt || new Date(session.startsAt).getTime() <= now;

  return (
    <div className="fade-in">
      <h1 style={{ fontSize: 21, fontWeight: 800, margin: '0 0 4px' }}>{session.collegeName || 'Session'}</h1>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px 16px', fontSize: 13, color: 'var(--color-muted)', marginBottom: 16 }}>
        {session.batchName && <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><MapPin size={13} /> {session.batchName}</span>}
        {session.startsAt && (
          <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
            <Clock size={13} /> {new Date(session.startsAt).toLocaleString(undefined, { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}
          </span>
        )}
      </div>

      <div className="card" style={{ padding: '14px 16px' }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 13, fontWeight: 700, marginBottom: 8 }}>
          <span>{summary.marked}/{summary.total} marked</span>
          <span style={{ color: 'var(--color-muted)', fontWeight: 500 }}>
            <span style={{ color: 'var(--color-success-txt)' }}>{summary.present} present</span>
            {'  ·  '}
            <span style={{ color: 'var(--color-danger-txt)' }}>{summary.absent} absent</span>
          </span>
        </div>
        <div className="progress-track"><div className="progress-fill" style={{ width: `${progressPct}%` }} /></div>
      </div>

      {!hasStarted ? (
        <div className="card empty-state starts-soon-card" style={{ marginTop: 20 }}>
          <div className="starts-soon-icon"><Hourglass size={26} /></div>
          <p style={{ fontWeight: 700, fontSize: 16, margin: '2px 0 4px' }}>This session hasn&apos;t started yet</p>
          <p className="starts-soon-countdown">{timeUntil(session.startsAt!, now)}</p>
          <p style={{ fontSize: 12.5, color: 'var(--color-muted)', margin: '10px 0 0' }}>
            Attendance marking opens automatically once it begins.
          </p>
        </div>
      ) : summary.total === 0 ? (
        <div className="card empty-state" style={{ marginTop: 20 }}>
          <p>This session has no roster attached.</p>
        </div>
      ) : (
        <>
          <div className="view-toggle">
            <button type="button" className={viewMode === 'stack' ? 'active' : ''} onClick={() => setViewMode('stack')}>
              <Layers size={14} /> Stack
            </button>
            <button type="button" className={viewMode === 'list' ? 'active' : ''} onClick={() => setViewMode('list')}>
              <ListIcon size={14} /> List
            </button>
          </div>

          {viewMode === 'stack' ? (
            <>
              {!top ? (
                <div className="card empty-state" style={{ marginTop: 12 }}>
                  <PartyPopper size={32} style={{ margin: '0 auto 12px', color: 'var(--color-success)' }} />
                  <p style={{ fontWeight: 700 }}>All students marked!</p>
                </div>
              ) : (
                <>
                  <div className="swipe-stage">
                    <AnimatePresence initial={false}>
                      {unmarked.slice(0, 2).reverse().map((s, idx, arr) => {
                        const isTop = idx === arr.length - 1;
                        return (
                          <motion.div
                            key={s.rollNumber}
                            className="swipe-card"
                            style={{ zIndex: isTop ? 2 : 1 }}
                            initial={{ scale: 0.94, opacity: 0 }}
                            animate={{ scale: isTop ? 1 : 0.96, opacity: 1, y: isTop ? 0 : 10 }}
                            exit={{ x: exitDir * 340, opacity: 0, rotate: exitDir * 12, transition: { duration: 0.25 } }}
                            drag={isTop ? 'x' : false}
                            dragElastic={0.7}
                            whileDrag={{ scale: 1.02 }}
                            onDragEnd={isTop ? (e, info) => handleDragEnd(s, e, info) : undefined}
                          >
                            <div className="swipe-card-avatar">{initials(s.name)}</div>
                            <div style={{ fontSize: 19, fontWeight: 700 }}>{s.name}</div>
                            <div style={{ fontSize: 14, color: 'var(--color-muted)', marginTop: 2 }}>{s.rollNumber}</div>
                          </motion.div>
                        );
                      })}
                    </AnimatePresence>
                  </div>

                  <div className="swipe-hint">
                    <span className="swipe-hint-present">&larr; Swipe left: Present</span>
                    <span className="swipe-hint-absent">Swipe right: Absent &rarr;</span>
                  </div>

                  <div className="action-row">
                    <button className="action-btn action-btn-present" onClick={() => top && markStudent(top, 'present')} disabled={pending}>
                      <Check size={20} /> Present
                    </button>
                    <button className="action-btn action-btn-absent" onClick={() => top && markStudent(top, 'absent')} disabled={pending}>
                      <X size={20} /> Absent
                    </button>
                  </div>
                </>
              )}

              {markedList.length > 0 && (
                <>
                  <div className="section-heading">Marked ({markedList.length})</div>
                  <div className="card">
                    {markedList.map((s) => (
                      <div className="marked-row" key={s.rollNumber}>
                        <div>
                          <div style={{ fontWeight: 600 }}>{s.name}</div>
                          <div style={{ fontSize: 12, color: 'var(--color-muted)' }}>
                            {s.rollNumber}
                            {s.source === 'self_submitted' && ' · Self-submitted'}
                          </div>
                        </div>
                        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                          <span className={`badge ${s.status === 'present' ? 'badge-success' : 'badge-danger'}`}>{s.status}</span>
                          {s.source === 'manual' && (
                            <button className="icon-btn" style={{ width: 30, height: 30 }} onClick={() => undoStudent(s)} aria-label="Undo mark" title="Undo">
                              <RotateCcw size={13} />
                            </button>
                          )}
                        </div>
                      </div>
                    ))}
                  </div>
                </>
              )}
            </>
          ) : (
            <>
              <div className="list-search">
                <Search size={15} className="list-search-icon" />
                <input
                  type="text"
                  placeholder="Search by name or roll number…"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  aria-label="Search students"
                />
              </div>

              {filteredStudents.length === 0 ? (
                <div className="card empty-state" style={{ marginTop: 12 }}>
                  <p>No students match your search.</p>
                </div>
              ) : (
                <div className="list-rows">
                  {filteredStudents.map((s) => (
                    <StudentRow key={s.rollNumber} student={s} onSwipe={markStudent} onUndo={undoStudent} disabled={pending} />
                  ))}
                </div>
              )}

              <p style={{ fontSize: 12, color: 'var(--color-muted)', textAlign: 'center', margin: '12px 0 4px' }}>
                Swipe left for present, right for absent — any student, any time.
              </p>
            </>
          )}
        </>
      )}
    </div>
  );
};

const StudentRow = ({
  student,
  onSwipe,
  onUndo,
  disabled,
}: {
  student: RosterStudent;
  onSwipe: (student: RosterStudent, status: 'present' | 'absent') => void;
  onUndo: (student: RosterStudent) => void;
  disabled: boolean;
}) => {
  // A self-submitted row is forensic student data — the manual-mark endpoint
  // 409s on it, so it isn't swipeable here either; shown read-only instead.
  const readOnly = student.source === 'self_submitted';
  const statusClass = student.status === 'present' ? ' list-row-present' : student.status === 'absent' ? ' list-row-absent' : '';

  return (
    <motion.div
      className={`list-row${statusClass}${readOnly ? ' list-row-readonly' : ''}`}
      drag={readOnly || disabled ? false : 'x'}
      dragConstraints={{ left: 0, right: 0 }}
      dragElastic={0.5}
      whileDrag={{ scale: 1.01 }}
      onDragEnd={(_e, info) => {
        if (info.offset.x < -SWIPE_THRESHOLD) onSwipe(student, 'present');
        else if (info.offset.x > SWIPE_THRESHOLD) onSwipe(student, 'absent');
      }}
    >
      <div className="list-row-avatar">{initials(student.name)}</div>
      <div className="list-row-info">
        <div className="list-row-name">{student.name}</div>
        <div className="list-row-roll">
          {student.rollNumber}
          {student.source === 'self_submitted' && ' · Self-submitted'}
        </div>
      </div>
      {student.source === 'manual' && (
        <button
          type="button"
          className="icon-btn"
          style={{ width: 30, height: 30, flexShrink: 0 }}
          onClick={(e) => { e.stopPropagation(); onUndo(student); }}
          aria-label={`Undo mark for ${student.name}`}
          title="Undo"
        >
          <RotateCcw size={13} />
        </button>
      )}
    </motion.div>
  );
};

export default SessionDetail;
