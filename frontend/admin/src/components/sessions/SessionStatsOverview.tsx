import { Calendar, CheckCircle, Percent, Users, XCircle } from 'lucide-react';

export interface SessionRecordsOverview {
  totalRecords: number;
  filteredRecords: number;
  present: number;
  absent: number;
  attendancePercent: number;
}

interface Props {
  overview: SessionRecordsOverview | null;
}

// Global counts for the currently active filters — scanned server-side with
// no page-size cap, so these stay accurate even once the sessions table
// itself is capped for display (see SESSIONS_LIST_PAGE_SIZE on the backend).
const SessionStatsOverview = ({ overview }: Props) => {
  return (
    <div className="session-records-stats">
      <div className="stat-card-new stat-card-total">
        <div className="stat-card-icon-wrapper"><Calendar size={28} /></div>
        <span className="stat-card-label">TOTAL DB RECORDS</span>
        <span className="stat-card-value">{overview?.totalRecords ?? 0}</span>
        <div className="stat-card-accent-line"></div>
      </div>

      <div className="stat-card-new stat-card-filtered">
        <div className="stat-card-icon-wrapper"><Users size={28} /></div>
        <span className="stat-card-label">FILTERED RECORDS</span>
        <span className="stat-card-value">{overview?.filteredRecords ?? 0}</span>
        <div className="stat-card-accent-line"></div>
      </div>

      <div className="stat-card-new stat-card-verified">
        <div className="stat-card-icon-wrapper"><CheckCircle size={28} /></div>
        <span className="stat-card-label">PRESENT</span>
        <span className="stat-card-value">{overview?.present ?? 0}</span>
        <div className="stat-card-accent-line"></div>
      </div>

      <div className="stat-card-new stat-card-unverified">
        <div className="stat-card-icon-wrapper"><XCircle size={28} /></div>
        <span className="stat-card-label">ABSENT</span>
        <span className="stat-card-value">{overview?.absent ?? 0}</span>
        <div className="stat-card-accent-line"></div>
      </div>

      <div className="stat-card-new stat-card-percent">
        <div className="stat-card-icon-wrapper"><Percent size={28} /></div>
        <span className="stat-card-label">ATTENDANCE</span>
        <span className="stat-card-value">{Math.round(overview?.attendancePercent ?? 0)}%</span>
        <div className="stat-card-accent-line"></div>
      </div>
    </div>
  );
};

export default SessionStatsOverview;
