import { useEffect, useState, useCallback, useRef } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { MapPin, ClipboardList, Users, Flag } from 'lucide-react';
import { motion } from 'framer-motion';
import PageHeader from '../components/ui/PageHeader';
import StatTile from '../components/ui/StatTile';
import { SkeletonTiles } from '../components/ui/Skeleton';
import AttendanceChart from '../components/ui/AttendanceChart';
import ErrorBoundary from '../components/ui/ErrorBoundary';

interface DashboardStats {
  totalLocations: number;
  activeSessions: number;
  totalAttendance: number;
  flaggedUnreviewed: number;
}

const Dashboard = () => {
  const [stats, setStats] = useState<DashboardStats | null>(null);
  const [loading, setLoading] = useState(true);
  const abortRef = useRef<AbortController | null>(null);

  const fetchStats = useCallback(async () => {
    abortRef.current?.abort();
    abortRef.current = new AbortController();
    try {
      const res = await axios.get<DashboardStats>('/api/admin/dashboard', { signal: abortRef.current.signal });
      setStats(res.data);
    } catch (error) {
      if (error instanceof Error && error.name !== 'CanceledError') toast.error('Failed to fetch stats');
    } finally {
      setLoading(false);
    }
  }, []); // abortRef is a mutable ref, so it doesn't need to be in the dependency array

  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, 30000);
    return () => { clearInterval(interval); abortRef.current?.abort(); };
  }, [fetchStats]);

  return (
    <div className="container">
      <PageHeader title="Dashboard" />

      {loading ? (
        <SkeletonTiles count={4} />
      ) : (
        <motion.div className="grid" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.3 }}>
          <StatTile label="Total Locations"    value={stats?.totalLocations ?? 0}    icon={MapPin}       linkTo="/locations" linkLabel="Manage" />
          <StatTile label="Active Sessions"    value={stats?.activeSessions ?? 0}    icon={ClipboardList} tone="success" linkTo="/sessions" linkLabel="View" />
          <StatTile label="Total Attendance"   value={stats?.totalAttendance ?? 0}   icon={Users} linkTo="/sessions" linkLabel="Details" />
          <StatTile label="Flagged Unreviewed" value={stats?.flaggedUnreviewed ?? 0} icon={Flag}  tone="danger" linkTo="/flagged" linkLabel="Review" />
        </motion.div>
      )}

      <div className="card mt-6">
        <ErrorBoundary fallback={<div className="chart-empty chart-error">Failed to render attendance chart.</div>}>
          <AttendanceChart />
        </ErrorBoundary>
      </div>
    </div>
  );
};

export default Dashboard;
