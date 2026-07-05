import { Link } from 'react-router-dom';

const StatTile = ({ label, value, icon: Icon, tone, linkTo, linkLabel }) => (
  <div className="stat-tile">
    <div className="stat-tile-top">
      <h3>{label}</h3>
      {Icon && (
        <div className={`stat-tile-icon${tone ? ` tone-${tone}` : ''}`}>
          <Icon size={18} />
        </div>
      )}
    </div>
    <div className={`stat-tile-value${tone ? ` tone-${tone}` : ''}`}>{value}</div>
    {linkTo && (
      <Link to={linkTo} className="btn btn-secondary btn-small">
        {linkLabel || 'View'}
      </Link>
    )}
  </div>
);

export default StatTile;
