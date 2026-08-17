import { useState } from 'react';
import type { FormEvent } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { UserPlus, X } from 'lucide-react';
import type { Mentor } from '../../pages/Sessions';

interface AdminUserResponse {
  _id: string;
  username: string;
  email: string;
  fullName?: string;
  role: string;
  isActive: boolean;
}

interface InlineMentorCreateProps {
  email: string;
  onCreated: (mentor: Mentor) => void;
  onCancel: () => void;
}

// Lets a super-admin create the missing mentor account right where they
// noticed it was missing (the Excel-upload review panel), instead of
// abandoning the upload flow to go create it on the User Management page
// and re-uploading. Mirrors UserManagement.tsx's create-account fields.
const InlineMentorCreate = ({ email, onCreated, onCancel }: InlineMentorCreateProps) => {
  const [username, setUsername] = useState(() => email.split('@')[0].replace(/[^a-zA-Z0-9]/g, '').slice(0, 30) || 'mentor');
  const [fullName, setFullName] = useState('');
  const [password, setPassword] = useState('');
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setSaving(true);
    try {
      const res = await axios.post<AdminUserResponse>('/api/admin/users', {
        username,
        email,
        fullName: fullName || null,
        password,
        role: 'admin',
      });
      toast.success(`Mentor account created for ${email}`);
      onCreated({
        _id: res.data._id,
        username: res.data.username,
        fullName: res.data.fullName,
        role: res.data.role,
        isActive: res.data.isActive,
        email: res.data.email,
      });
    } catch (error) {
      const err = error as { response?: { data?: { message?: string } } };
      toast.error(err.response?.data?.message || 'Failed to create mentor account');
    } finally {
      setSaving(false);
    }
  };

  return (
    <form
      onSubmit={handleSubmit}
      style={{
        border: '1px dashed var(--color-primary)',
        borderRadius: 'var(--radius-sm)',
        padding: '0.75rem',
        marginTop: '0.5rem',
        background: 'var(--color-primary-subtle, var(--color-primary-light))',
        display: 'flex',
        flexDirection: 'column',
        gap: '0.5rem',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <strong style={{ fontSize: '0.82rem', display: 'flex', alignItems: 'center', gap: 6 }}>
          <UserPlus size={14} /> Create mentor account for {email}
        </strong>
        <button type="button" onClick={onCancel} aria-label="Cancel" style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--color-muted)' }}>
          <X size={14} />
        </button>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '0.5rem' }}>
        <input
          type="text"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          placeholder="Username"
          minLength={3}
          maxLength={30}
          required
        />
        <input
          type="text"
          value={fullName}
          onChange={(e) => setFullName(e.target.value)}
          placeholder="Full name (optional)"
        />
      </div>
      <input
        type="password"
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        placeholder="Set a password (min 6 characters)"
        minLength={6}
        required
      />
      <div style={{ display: 'flex', gap: '0.5rem' }}>
        <button type="submit" className="btn btn-success btn-small" disabled={saving}>
          {saving ? 'Creating…' : 'Create Account'}
        </button>
        <button type="button" className="btn btn-secondary btn-small" onClick={onCancel} disabled={saving}>
          Cancel
        </button>
      </div>
    </form>
  );
};

export default InlineMentorCreate;
