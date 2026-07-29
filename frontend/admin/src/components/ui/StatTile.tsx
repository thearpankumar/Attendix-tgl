import { Link } from 'react-router';
import type { ElementType } from 'react';

interface StatTileProps {
  label: string;
  value: number | string;
  icon?: ElementType;
  tone?: string;
  linkTo?: string;
  linkLabel?: string;
}

const StatTile = ({ label, value, icon: Icon, tone, linkTo, linkLabel }: StatTileProps) => (
  <div className="stat-tile">
    <div className="stat-tile-header">
      <span className="stat-tile-label">{label}</span>
      {Icon && (
        <div className={`stat-tile-icon${tone ? ` tone-${tone}` : ''}`}>
          <Icon size={20} />
        </div>
      )}
    </div>
    <div className={`stat-tile-value${tone ? ` tone-${tone}` : ''}`}>{value}</div>
    {linkTo && (
      <Link to={linkTo} className="stat-tile-link">{linkLabel || 'View'} →</Link>
    )}
  </div>
);

export default StatTile;
