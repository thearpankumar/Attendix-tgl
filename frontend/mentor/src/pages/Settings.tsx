import { useState } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { Lock, Loader2 } from 'lucide-react';
import { useAuth } from '../context/AuthContext';

const Settings = () => {
  const { admin } = useAuth();
  const [currentPassword, setCurrentPassword] = useState('');
  const [newPassword, setNewPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (newPassword.length < 6) { toast.error('New password must be at least 6 characters'); return; }
    if (newPassword !== confirmPassword) { toast.error('New passwords do not match'); return; }
    setSaving(true);
    try {
      await axios.patch('/api/admin/profile/password', { currentPassword, newPassword });
      toast.success('Password updated');
      setCurrentPassword(''); setNewPassword(''); setConfirmPassword('');
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to update password');
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fade-in">
      <h1 style={{ fontSize: 21, fontWeight: 800, margin: '0 0 4px' }}>Settings</h1>
      <p style={{ fontSize: 13, color: 'var(--color-muted)', marginBottom: 20 }}>
        Signed in as <strong>{admin?.fullName || admin?.username}</strong> ({admin?.email})
        {admin?.collegeName ? ` · ${admin.collegeName}` : ''}
      </p>

      <div className="card" style={{ padding: '18px 16px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 16 }}>
          <Lock size={16} color="var(--color-primary)" />
          <span style={{ fontWeight: 700, fontSize: 15 }}>Change Password</span>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="field">
            <label>Current Password</label>
            <input type="password" value={currentPassword} onChange={(e) => setCurrentPassword(e.target.value)} required />
          </div>
          <div className="field">
            <label>New Password</label>
            <input type="password" value={newPassword} onChange={(e) => setNewPassword(e.target.value)} minLength={6} required />
          </div>
          <div className="field">
            <label>Confirm New Password</label>
            <input type="password" value={confirmPassword} onChange={(e) => setConfirmPassword(e.target.value)} minLength={6} required />
          </div>
          <button type="submit" className="btn btn-primary btn-block" disabled={saving}>
            {saving ? <Loader2 size={16} className="spin" /> : null}
            {saving ? 'Saving…' : 'Update Password'}
          </button>
        </form>
      </div>
    </div>
  );
};

export default Settings;
