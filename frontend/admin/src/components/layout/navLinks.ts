import { LayoutDashboard, MapPin, ClipboardList, Link2, Fingerprint, Flag } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

export interface NavLink {
  to: string;
  label: string;
  icon: LucideIcon;
  end?: boolean;
  danger?: boolean;
}

export const navLinks: NavLink[] = [
  { to: '/',           label: 'Dashboard',          icon: LayoutDashboard, end: true },
  { to: '/locations',  label: 'Locations',           icon: MapPin },
  { to: '/sessions',   label: 'Sessions',            icon: ClipboardList },
  { to: '/shortlinks', label: 'Short Links',         icon: Link2 },
  { to: '/webauthn',   label: 'WebAuthn Credentials',icon: Fingerprint },
  { to: '/flagged',    label: 'Flagged Records',      icon: Flag, danger: true },
];
