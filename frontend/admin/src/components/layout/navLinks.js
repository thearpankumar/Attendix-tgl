import { LayoutDashboard, MapPin, ClipboardList, Link2, Fingerprint, Flag } from 'lucide-react';

export const navLinks = [
  { to: '/', label: 'Dashboard', icon: LayoutDashboard, end: true },
  { to: '/locations', label: 'Locations', icon: MapPin },
  { to: '/sessions', label: 'Sessions', icon: ClipboardList },
  { to: '/shortlinks', label: 'Short Links', icon: Link2 },
  { to: '/webauthn', label: 'Biometrics', icon: Fingerprint },
  { to: '/flagged', label: 'Flags', icon: Flag, danger: true },
];
