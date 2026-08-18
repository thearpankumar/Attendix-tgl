import { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { useNavigate } from 'react-router';
import { toast } from 'react-toastify';
import { MapPin, Clock, Users, ClipboardX, Loader2, Activity, CalendarClock, GraduationCap, PieChart } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import GreetingBanner from '../components/GreetingBanner';

interface Session {
  _id: string;
  locationName?: string;
  batchName?: string;
  collegeName?: string;
  startsAt?: string;
  expiresAt: string;
  isActive: boolean;
  attendanceCount: number;
  description?: string;
}

interface RosterSummary {
  total: number;
  marked: number;
  present: number;
  absent: number;
  unmarked: number;
}

type SessionState = 'live' | 'upcoming' | 'closed';

const sessionState = (s: Session): SessionState => {
  const now = Date.now();
  if (!s.isActive || new Date(s.expiresAt).getTime() < now) return 'closed';
  if (s.startsAt && new Date(s.startsAt).getTime() > now) return 'upcoming';
  return 'live';
};

const formatSlot = (s: Session) => {
  if (!s.startsAt) return null;
  return new Date(s.startsAt).toLocaleString(undefined, {
    weekday: 'short', month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit',
  });
};

const timeUntil = (iso: string) => {
  const diffMin = Math.round((new Date(iso).getTime() - Date.now()) / 60000);
  if (diffMin <= 0) return 'Starting now';
  const h = Math.floor(diffMin / 60);
  const m = diffMin % 60;
  return h > 0 ? `Starts in ${h}h ${m}m` : `Starts in ${m}m`;
};

// Calendar-day comparison (local time), not a millisecond/24h-window
// comparison — a session starting at 11pm today and one starting at 1am
// tomorrow are ~2 hours apart but must land in different buckets, while a
// session 20+ hours from now that's still "today" (started this calendar
// day) must not get pushed into "Upcoming".
const isToday = (iso: string) => {
  const d = new Date(iso);
  const now = new Date();
  return d.getFullYear() === now.getFullYear() && d.getMonth() === now.getMonth() && d.getDate() === now.getDate();
};

const Dashboard = () => {
  const navigate = useNavigate();
  const [sessions, setSessions] = useState<Session[]>([]);
  const [rosterMap, setRosterMap] = useState<Record<string, RosterSummary>>({});
  const [loading, setLoading] = useState(true);
  const [sessionsTab, setSessionsTab] = useState<'today' | 'upcoming'>('today');

  const fetchSessions = useCallback(async () => {
    try {
      const res = await axios.get<Session[]>('/api/admin/sessions');
      setSessions(res.data);

      const open = res.data.filter((s) => sessionState(s) !== 'closed');
      const entries = await Promise.all(open.map(async (s) => {
        try {
          const r = await axios.get<{ summary: RosterSummary }>(`/api/admin/sessions/${s._id}/roster`);
          return [s._id, r.data.summary] as const;
        } catch {
          return [s._id, null] as const;
        }
      }));
      setRosterMap(Object.fromEntries(entries.filter((e): e is [string, RosterSummary] => e[1] !== null)));
    } catch {
      toast.error('Failed to load your sessions');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSessions();
    const interval = setInterval(fetchSessions, 30000);
    return () => clearInterval(interval);
  }, [fetchSessions]);

  if (loading) {
    return (
      <div className="loading-screen" style={{ minHeight: '50vh' }}>
        <Loader2 size={22} className="spin" />
      </div>
    );
  }

  const openSessions = sessions.filter((s) => sessionState(s) !== 'closed');
  const liveSessions = openSessions.filter((s) => sessionState(s) === 'live');
  const upcomingSessions = openSessions.filter((s) => sessionState(s) === 'upcoming');
  const closedSessions = sessions.filter((s) => sessionState(s) === 'closed');

  // "Today" = currently live, or scheduled to start later today. "Upcoming"
  // = scheduled for a future calendar day. A live session is always today
  // by definition (it's happening right now), regardless of when it started.
  const todaySessions = openSessions.filter((s) => sessionState(s) === 'live' || (s.startsAt && isToday(s.startsAt)));
  const upcomingDaySessions = openSessions.filter((s) => sessionState(s) === 'upcoming' && s.startsAt && !isToday(s.startsAt));
  const visibleSessions = sessionsTab === 'today' ? todaySessions : upcomingDaySessions;

  const scheduled = todaySessions
    .filter((s) => s.startsAt)
    .sort((a, b) => new Date(a.startsAt!).getTime() - new Date(b.startsAt!).getTime());

  // "Students" is a headcount across everything on your plate (today +
  // upcoming), so it stays scoped to openSessions. "Attendance" is different:
  // it's meant to answer "how is marking going right now" — rolling in
  // sessions that haven't started yet (where marked is 0, or occasionally a
  // handful of stray marks from before this session was rescheduled) can
  // make a tiny, unrepresentative sample of marks look like "100%" even
  // though thousands of students are simply unmarked. Scoping the
  // present/marked ratio to only currently-live sessions means "no live
  // sessions" correctly shows "—" instead of a misleading percentage.
  const rosterTotals = openSessions.reduce(
    (acc, s) => {
      const r = rosterMap[s._id];
      if (r) { acc.total += r.total; }
      return acc;
    },
    { total: 0 }
  );
  const liveRosterTotals = liveSessions.reduce(
    (acc, s) => {
      const r = rosterMap[s._id];
      if (r) { acc.present += r.present; acc.marked += r.marked; }
      return acc;
    },
    { present: 0, marked: 0 }
  );
  const attendancePct = liveRosterTotals.marked === 0 ? null : Math.round((liveRosterTotals.present / liveRosterTotals.marked) * 100);

  return (
    <div className="fade-in">
      <GreetingBanner />

      {sessions.length === 0 ? (
        <div className="card empty-state" style={{ marginTop: 20 }}>
          <ClipboardX size={32} style={{ margin: '0 auto 12px', color: 'var(--color-faint)' }} />
          <p>No sessions assigned to you yet.</p>
          <p style={{ fontSize: 13 }}>Ask your super admin to assign you to a session.</p>
        </div>
      ) : (
        <>
          <div className="stat-grid">
            <StatCard icon={Activity} label="Active Sessions" value={liveSessions.length} tone="primary" />
            <StatCard icon={CalendarClock} label="Upcoming" value={upcomingSessions.length} tone="warning" />
            <StatCard icon={GraduationCap} label="Students" value={rosterTotals.total} tone="success" />
            <StatCard icon={PieChart} label="Attendance" value={attendancePct === null ? '—' : `${attendancePct}%`} tone="primary" />
          </div>

          {openSessions.length > 0 && (
            <div className="panel">
              <div className="panel-header">Your Sessions</div>
              <div className="view-toggle" style={{ margin: '12px 16px 4px' }}>
                <button type="button" className={sessionsTab === 'today' ? 'active' : ''} onClick={() => setSessionsTab('today')}>
                  Today{todaySessions.length > 0 ? ` (${todaySessions.length})` : ''}
                </button>
                <button type="button" className={sessionsTab === 'upcoming' ? 'active' : ''} onClick={() => setSessionsTab('upcoming')}>
                  Upcoming{upcomingDaySessions.length > 0 ? ` (${upcomingDaySessions.length})` : ''}
                </button>
              </div>
              {visibleSessions.length === 0 ? (
                <div className="empty-state" style={{ padding: '28px 20px' }}>
                  <p style={{ fontSize: 13 }}>
                    {sessionsTab === 'today' ? 'No sessions today.' : 'No upcoming sessions scheduled.'}
                  </p>
                </div>
              ) : (
                <div>
                  {visibleSessions.map((s) => {
                    const state = sessionState(s);
                    const roster = rosterMap[s._id];
                    const slot = sessionsTab === 'upcoming' ? formatSlot(s) : null;
                    return (
                      <div key={s._id} className="session-row" onClick={() => navigate(`/sessions/${s._id}`)}>
                        <div>
                          <div className="session-row-title">{s.batchName || s.collegeName || 'Session'}</div>
                          <div className="session-row-sub">
                            {[s.collegeName, s.locationName].filter(Boolean).join(' · ') || 'No location'}
                            {slot ? ` · ${slot}` : ''}
                          </div>
                        </div>
                        <div className="session-row-meta">
                          <span className={`badge ${state === 'live' ? 'badge-success' : 'badge-warning'}`}>
                            {state === 'live' ? 'LIVE' : (s.startsAt ? timeUntil(s.startsAt) : 'Upcoming')}
                          </span>
                          <span className="session-row-count">
                            {roster ? `${roster.marked}/${roster.total}` : `${s.attendanceCount}`} Marked
                          </span>
                        </div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          )}

          {scheduled.length > 0 && (
            <div className="panel">
              <div className="panel-header">Today&apos;s Schedule</div>
              <div className="timeline">
                {scheduled.map((s) => {
                  const state = sessionState(s);
                  return (
                    <div key={s._id} className="timeline-item">
                      <div className="timeline-marker">
                        <span className={`timeline-dot${state === 'live' ? ' live' : ''}`} />
                      </div>
                      <div>
                        <div className="timeline-time">
                          {new Date(s.startsAt!).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })}
                          {state === 'upcoming' && ' · Upcoming'}
                        </div>
                        <div className="timeline-title">{s.batchName || s.collegeName || 'Session'}</div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {liveRosterTotals.marked > 0 && (
            <div className="panel">
              <div className="panel-header">Attendance Overview</div>
              <div className="donut-wrap">
                <div
                  className="donut"
                  style={{ background: `conic-gradient(var(--color-success) 0% ${attendancePct}%, var(--color-danger) ${attendancePct}% 100%)` }}
                >
                  <div className="donut-center">
                    <div className="donut-pct">{attendancePct}%</div>
                    <div className="donut-label">Present</div>
                  </div>
                </div>
                <div className="donut-legend">
                  <div><span className="legend-dot present" /> Present <strong>{liveRosterTotals.present}</strong></div>
                  <div><span className="legend-dot absent" /> Absent <strong>{liveRosterTotals.marked - liveRosterTotals.present}</strong></div>
                </div>
              </div>
            </div>
          )}

          {closedSessions.length > 0 && (
            <>
              <div className="section-heading">Past / Inactive</div>
              {closedSessions.map((s) => <SessionCard key={s._id} session={s} muted onClick={() => navigate(`/sessions/${s._id}`)} />)}
            </>
          )}
        </>
      )}
    </div>
  );
};

const StatCard = ({ icon: Icon, label, value, tone }: { icon: LucideIcon; label: string; value: number | string; tone: 'primary' | 'success' | 'warning' }) => (
  <div className="stat-card">
    <div className={`stat-card-icon tone-${tone}`}><Icon size={17} /></div>
    <div>
      <div className="stat-card-value">{value}</div>
      <div className="stat-card-label">{label}</div>
    </div>
  </div>
);

const SessionCard = ({ session, onClick, muted }: { session: Session; onClick: () => void; muted?: boolean }) => {
  const slot = formatSlot(session);
  return (
    <div className="card session-card" onClick={onClick} style={{ opacity: muted ? 0.65 : 1 }}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 8 }}>
        <div>
          <div style={{ fontSize: 17, fontWeight: 700 }}>{session.collegeName || session.locationName || 'Session'}</div>
          {session.batchName && <div style={{ fontSize: 13, color: 'var(--color-muted)', marginTop: 2 }}>{session.batchName}</div>}
        </div>
        <span className="badge badge-neutral">Closed</span>
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '10px 16px', marginTop: 14, fontSize: 13, color: 'var(--color-muted)' }}>
        {slot && <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Clock size={14} /> {slot}</span>}
        {session.locationName && <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><MapPin size={14} /> {session.locationName}</span>}
        <span style={{ display: 'flex', alignItems: 'center', gap: 5 }}><Users size={14} /> {session.attendanceCount} marked</span>
      </div>
    </div>
  );
};

export default Dashboard;
