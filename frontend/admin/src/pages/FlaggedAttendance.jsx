import React, { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';

const FlaggedAttendance = () => {
  const [flaggedRecords, setFlaggedRecords] = useState([]);
  const [loading, setLoading] = useState(true);
  const [filter, setFilter] = useState({
    flagType: '',
    reviewed: 'all',
  });

  const fetchData = useCallback(async () => {
    try {
      const params = new URLSearchParams();
      if (filter.flagType) params.append('flagType', filter.flagType);
      
      const res = await axios.get(`/api/admin/flagged?${params.toString()}`);
      
      let records = res.data;
      
      if (filter.reviewed === 'reviewed') {
        records = records.filter(r => r.flagReviewed);
      } else if (filter.reviewed === 'unreviewed') {
        records = records.filter(r => !r.flagReviewed);
      }
      
      setFlaggedRecords(records);
    } catch (error) {
      toast.error('Failed to fetch flagged records');
    } finally {
      setLoading(false);
    }
  }, [filter]);

  useEffect(() => {
    fetchData();
  }, [fetchData]);

  const handleReview = async (id, reviewed) => {
    try {
      await axios.patch(`/api/admin/attendance/${id}/review`, { reviewed });
      toast.success(reviewed ? 'Flag marked as reviewed' : 'Flag review removed');
      fetchData();
    } catch (error) {
      toast.error('Failed to update review status');
    }
  };

  const getFlagColor = (flagType) => {
    switch (flagType) {
      case 'MULTI_STUDENT_DEVICE':
        return '#e74c3c';
      case 'STUDENT_DEVICE_SWITCHED':
        return '#f39c12';
      case 'RAPID_SUBMISSION':
        return '#9b59b6';
      default:
        return '#95a5a6';
    }
  };

  if (loading) return <div className="loading">Loading...</div>;

  return (
    <div className="container">
      <h2 style={{ marginBottom: '20px' }}>Flagged Attendance Records</h2>
      
      <div className="card" style={{ marginBottom: '20px' }}>
        <div style={{ display: 'flex', gap: '20px', alignItems: 'center', flexWrap: 'wrap' }}>
          <div>
            <label style={{ marginRight: '10px' }}>Flag Type:</label>
            <select
              value={filter.flagType}
              onChange={(e) => setFilter({ ...filter, flagType: e.target.value })}
              style={{ padding: '8px', borderRadius: '4px', border: '1px solid #ddd' }}
            >
              <option value="">All Flags</option>
              <option value="MULTI_STUDENT_DEVICE">Multi-Student Device</option>
              <option value="STUDENT_DEVICE_SWITCHED">Device Switched</option>
              <option value="RAPID_SUBMISSION">Rapid Submission</option>
            </select>
          </div>
          
          <div>
            <label style={{ marginRight: '10px' }}>Review Status:</label>
            <select
              value={filter.reviewed}
              onChange={(e) => setFilter({ ...filter, reviewed: e.target.value })}
              style={{ padding: '8px', borderRadius: '4px', border: '1px solid #ddd' }}
            >
              <option value="all">All</option>
              <option value="unreviewed">Unreviewed</option>
              <option value="reviewed">Reviewed</option>
            </select>
          </div>
        </div>
      </div>

      {flaggedRecords.length === 0 ? (
        <div className="card">
          <p style={{ textAlign: 'center', color: '#666' }}>
            No flagged attendance records found.
          </p>
        </div>
      ) : (
        <div className="card">
          <table className="table">
            <thead>
              <tr>
                <th>Student</th>
                <th>Roll No</th>
                <th>Flag Type</th>
                <th>Session</th>
                <th>Time</th>
                <th>Status</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {flaggedRecords.map((record) => (
                <tr key={record._id}>
                  <td>{record.studentName}</td>
                  <td>{record.rollNumber}</td>
                  <td>
                    <span style={{
                      background: getFlagColor(record.deviceFlag),
                      color: 'white',
                      padding: '4px 8px',
                      borderRadius: '4px',
                      fontSize: '12px',
                    }}>
                      {record.deviceFlag?.replace(/_/g, ' ')}
                    </span>
                  </td>
                  <td style={{ fontSize: '13px' }}>
                    {record.sessionId?.description || 'Session'}
                    <div style={{ color: '#999', fontSize: '11px' }}>
                      {record.sessionId?._id?.substring(0, 8)}...
                    </div>
                  </td>
                  <td style={{ fontSize: '13px' }}>
                    {new Date(record.capturedAt).toLocaleString()}
                  </td>
                  <td>
                    {record.flagReviewed ? (
                      <span className="badge badge-success">Reviewed</span>
                    ) : (
                      <span className="badge badge-warning">Pending</span>
                    )}
                  </td>
                  <td>
                    <div className="actions-cell">
                      {!record.flagReviewed ? (
                        <button
                          className="btn btn-success btn-small"
                          onClick={() => handleReview(record._id, true)}
                        >
                          Mark Reviewed
                        </button>
                      ) : (
                        <button
                          className="btn btn-secondary btn-small"
                          onClick={() => handleReview(record._id, false)}
                        >
                          Unmark
                        </button>
                      )}
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      
      <div style={{ marginTop: '20px' }}>
        <h4>Flag Types Explained:</h4>
        <ul style={{ marginLeft: '20px', lineHeight: '2' }}>
          <li>
            <strong style={{ color: '#e74c3c' }}>Multi-Student Device:</strong> 
            Same device was used by multiple students
          </li>
          <li>
            <strong style={{ color: '#f39c12' }}>Device Switched:</strong> 
            Student used a different device than their registered device
          </li>
          <li>
            <strong style={{ color: '#9b59b6' }}>Rapid Submission:</strong> 
            Multiple submissions within 10 seconds
          </li>
        </ul>
      </div>
    </div>
  );
};

export default FlaggedAttendance;
