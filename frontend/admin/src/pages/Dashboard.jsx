import React, { useEffect, useState, useCallback, useRef } from 'react';
import axios from 'axios';
import { Link } from 'react-router-dom';
import { toast } from 'react-toastify';
import { MapPin, ClipboardList, Users } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import StatTile from '../components/ui/StatTile';
import { SkeletonTiles } from '../components/ui/Skeleton';

const Dashboard = () => {
  const [stats, setStats] = useState(null);
  const [loading, setLoading] = useState(true);
  const abortControllerRef = useRef(null);

  const fetchStats = useCallback(async () => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    abortControllerRef.current = new AbortController();

    try {
      const res = await axios.get('/api/admin/dashboard', {
        signal: abortControllerRef.current.signal,
      });
      setStats(res.data);
    } catch (error) {
      if (error.name !== 'CanceledError') {
        toast.error('Failed to fetch stats');
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchStats();
    const interval = setInterval(fetchStats, 30000);
    return () => {
      clearInterval(interval);
      if (abortControllerRef.current) {
        abortControllerRef.current.abort();
      }
    };
  }, [fetchStats]);

  return (
    <div className="container">
      <PageHeader title="Dashboard" />

      {loading ? (
        <SkeletonTiles count={3} />
      ) : (
        <div className="grid">
          <StatTile
            label="Total Locations"
            value={stats?.totalLocations || 0}
            icon={MapPin}
            linkTo="/locations"
            linkLabel="Manage"
          />
          <StatTile
            label="Active Sessions"
            value={stats?.activeSessions || 0}
            icon={ClipboardList}
            tone="success"
            linkTo="/sessions"
            linkLabel="View"
          />
          <StatTile
            label="Total Attendance"
            value={stats?.totalAttendance || 0}
            icon={Users}
          />
        </div>
      )}

      <div className="card">
        <h3 style={{ marginBottom: '15px' }}>Quick Actions</h3>
        <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
          <Link to="/locations" className="btn btn-primary">
            Add Location
          </Link>
          <Link to="/sessions" className="btn btn-success">
            Create Session
          </Link>
        </div>
      </div>
    </div>
  );
};

export default Dashboard;
