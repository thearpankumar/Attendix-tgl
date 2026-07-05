import React, { useEffect, useState, useCallback } from 'react';
import axios from 'axios';
import { toast } from 'react-toastify';
import { MapPin, LocateFixed } from 'lucide-react';
import PageHeader from '../components/ui/PageHeader';
import DataTable from '../components/ui/DataTable';
import Modal from '../components/ui/Modal';
import EmptyState from '../components/ui/EmptyState';
import Badge from '../components/ui/Badge';
import { SkeletonRows } from '../components/ui/Skeleton';

const Locations = () => {
  const [locations, setLocations] = useState([]);
  const [loading, setLoading] = useState(true);
  const [showModal, setShowModal] = useState(false);
  const [editingLocation, setEditingLocation] = useState(null);
  const [geoLoading, setGeoLoading] = useState(false);
  const [geoError, setGeoError] = useState('');
  const [currentCoords, setCurrentCoords] = useState(null);
  const [placeName, setPlaceName] = useState('');
  const [placeNameLoading, setPlaceNameLoading] = useState(false);
  const [manualCoords, setManualCoords] = useState({ latitude: '', longitude: '' });
  const [formData, setFormData] = useState({
    name: '',
    radiusMeters: 100,
    description: '',
  });

  const fetchLocations = useCallback(async () => {
    try {
      const res = await axios.get('/api/admin/locations');
      setLocations(res.data);
    } catch (error) {
      toast.error('Failed to fetch locations');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchLocations();
  }, [fetchLocations]);

  // Nominatim (OpenStreetMap) reverse geocoding — free, no API key
  const fetchPlaceName = async (lat, lon) => {
    setPlaceNameLoading(true);
    setPlaceName('');
    try {
      const res = await fetch(
        `https://nominatim.openstreetmap.org/reverse?format=json&lat=${lat}&lon=${lon}&zoom=16&addressdetails=1`,
        { headers: { 'Accept-Language': 'en' } }
      );
      if (!res.ok) throw new Error('Nominatim error');
      const data = await res.json();
      const a = data.address || {};
      const parts = [
        a.building || a.amenity || a.road || a.pedestrian,
        a.suburb || a.neighbourhood,
        a.city || a.town || a.village || a.county,
        a.state,
      ].filter(Boolean);
      setPlaceName(parts.length ? parts.join(', ') : data.display_name || 'Unknown location');
    } catch {
      setPlaceName('Could not fetch place name');
    } finally {
      setPlaceNameLoading(false);
    }
  };

  const getCurrentLocation = () => {
    setGeoLoading(true);
    setGeoError('');

    if (!navigator.geolocation) {
      setGeoError('Geolocation is not supported by this browser');
      setGeoLoading(false);
      return;
    }

    navigator.geolocation.getCurrentPosition(
      (position) => {
        const lat = position.coords.latitude;
        const lon = position.coords.longitude;
        const coords = { latitude: lat, longitude: lon, accuracy: position.coords.accuracy };
        setCurrentCoords(coords);
        setManualCoords({ latitude: lat.toFixed(7), longitude: lon.toFixed(7) });
        setGeoLoading(false);
        fetchPlaceName(lat, lon);
        toast.success('Location detected! Accuracy: ' + Math.round(position.coords.accuracy) + 'm');
      },
      (error) => {
        setGeoLoading(false);
        let message;
        switch (error.code) {
          case error.PERMISSION_DENIED:
            message = 'Location permission denied. You can enter coordinates manually below.';
            break;
          case error.POSITION_UNAVAILABLE:
            message = 'Location unavailable. You can enter coordinates manually below.';
            break;
          case error.TIMEOUT:
            message = 'Location request timed out. You can enter coordinates manually below.';
            break;
          default:
            message = 'Could not get location. Enter coordinates manually below.';
        }
        setGeoError(message);
      },
      { enableHighAccuracy: true, timeout: 15000, maximumAge: 0 }
    );
  };

  const handleManualCoordChange = (field, value) => {
    const updated = { ...manualCoords, [field]: value };
    setManualCoords(updated);
    const lat = parseFloat(field === 'latitude' ? value : updated.latitude);
    const lon = parseFloat(field === 'longitude' ? value : updated.longitude);
    if (!isNaN(lat) && !isNaN(lon) && lat >= -90 && lat <= 90 && lon >= -180 && lon <= 180) {
      setCurrentCoords({ latitude: lat, longitude: lon, accuracy: null });
    } else {
      setCurrentCoords(null);
      setPlaceName('');
    }
  };

  const handleCoordBlur = () => {
    if (currentCoords) {
      fetchPlaceName(currentCoords.latitude, currentCoords.longitude);
    }
  };

  const handleSubmit = async (e) => {
    e.preventDefault();

    if (!currentCoords) {
      toast.error('Please detect your location first');
      return;
    }

    try {
      const data = {
        name: formData.name,
        latitude: currentCoords.latitude,
        longitude: currentCoords.longitude,
        radiusMeters: parseInt(formData.radiusMeters),
        description: formData.description,
      };

      if (editingLocation) {
        await axios.put(`/api/admin/locations/${editingLocation._id}`, data);
        toast.success('Location updated');
      } else {
        await axios.post('/api/admin/locations', data);
        toast.success('Location created');
      }

      setShowModal(false);
      setEditingLocation(null);
      setCurrentCoords(null);
      setFormData({
        name: '',
        radiusMeters: 100,
        description: '',
      });
      fetchLocations();
    } catch (error) {
      toast.error(error.response?.data?.message || 'Failed to save location');
    }
  };

  const handleDelete = async (id) => {
    if (!window.confirm('Delete this location?')) return;
    try {
      await axios.delete(`/api/admin/locations/${id}`);
      toast.success('Location deleted');
      fetchLocations();
    } catch (error) {
      toast.error('Failed to delete location');
    }
  };

  const openEditModal = (location) => {
    setEditingLocation(location);
    const lat = location.latitude;
    const lon = location.longitude;
    setCurrentCoords({ latitude: lat, longitude: lon, accuracy: 0 });
    setManualCoords({ latitude: lat.toFixed(7), longitude: lon.toFixed(7) });
    setFormData({
      name: location.name,
      radiusMeters: location.radiusMeters,
      description: location.description || '',
    });
    setGeoError('');
    setPlaceName('');
    setShowModal(true);
    fetchPlaceName(lat, lon);
  };

  const openNewModal = () => {
    setEditingLocation(null);
    setCurrentCoords(null);
    setManualCoords({ latitude: '', longitude: '' });
    setFormData({ name: '', radiusMeters: 100, description: '' });
    setGeoError('');
    setPlaceName('');
    setShowModal(true);
  };

  const columns = [
    { key: 'name', label: 'Name', priority: 1, render: (loc) => loc.name },
    {
      key: 'coords',
      label: 'Coordinates',
      priority: 2,
      render: (loc) => `${loc.latitude.toFixed(6)}, ${loc.longitude.toFixed(6)}`,
    },
    { key: 'radius', label: 'Radius (m)', priority: 3, render: (loc) => loc.radiusMeters },
    {
      key: 'status',
      label: 'Status',
      priority: 1,
      render: (loc) => <Badge tone={loc.isActive ? 'success' : 'danger'}>{loc.isActive ? 'Active' : 'Inactive'}</Badge>,
    },
    {
      key: 'actions',
      label: 'Actions',
      priority: 1,
      render: (loc) => (
        <div className="actions-cell">
          <button className="btn btn-secondary btn-small" onClick={() => openEditModal(loc)}>
            Edit
          </button>
          <button className="btn btn-danger btn-small" onClick={() => handleDelete(loc._id)}>
            Delete
          </button>
        </div>
      ),
    },
  ];

  return (
    <div className="container">
      <PageHeader title="Locations">
        <button className="btn btn-primary" onClick={openNewModal}>
          Add Location
        </button>
      </PageHeader>

      {loading ? (
        <SkeletonRows />
      ) : locations.length === 0 ? (
        <EmptyState icon={MapPin} title="No locations yet" message="Create your first location to get started." />
      ) : (
        <div className="card">
          <DataTable columns={columns} rows={locations} rowKey={(loc) => loc._id} />
        </div>
      )}

      <Modal
        open={showModal}
        onClose={() => setShowModal(false)}
        title={editingLocation ? 'Edit Location' : 'Add Location'}
      >
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>Location Name *</label>
            <input
              type="text"
              value={formData.name}
              onChange={(e) => setFormData({ ...formData, name: e.target.value })}
              required
              placeholder="e.g., Main Lecture Hall"
            />
          </div>

          <div className="form-group">
            <label>Coordinates *</label>

            <button
              type="button"
              className="btn btn-secondary"
              onClick={getCurrentLocation}
              disabled={geoLoading}
              style={{ width: '100%', marginBottom: '10px' }}
            >
              <LocateFixed size={16} />
              {geoLoading ? 'Detecting Location...' : 'Auto-Detect My Location'}
            </button>

            <div className="form-row">
              <div>
                <label style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Latitude</label>
                <input
                  type="number"
                  step="any"
                  value={manualCoords.latitude}
                  onChange={(e) => handleManualCoordChange('latitude', e.target.value)}
                  onBlur={handleCoordBlur}
                  placeholder="e.g., 12.9716"
                  min="-90"
                  max="90"
                />
              </div>
              <div>
                <label style={{ fontSize: '12px', color: 'var(--color-text-muted)' }}>Longitude</label>
                <input
                  type="number"
                  step="any"
                  value={manualCoords.longitude}
                  onChange={(e) => handleManualCoordChange('longitude', e.target.value)}
                  onBlur={handleCoordBlur}
                  placeholder="e.g., 77.5946"
                  min="-180"
                  max="180"
                />
              </div>
            </div>

            {(placeNameLoading || placeName) && (
              <div className={`geo-banner ${placeNameLoading ? 'geo-banner--info' : 'geo-banner--success'}`}>
                {placeNameLoading ? (
                  'Fetching place name...'
                ) : (
                  <>
                    <MapPin size={13} style={{ verticalAlign: 'text-bottom', marginRight: '4px' }} />
                    {placeName}
                  </>
                )}
              </div>
            )}

            {currentCoords?.accuracy > 0 && (
              <div className="geo-accuracy">GPS Accuracy: ±{Math.round(currentCoords.accuracy)}m</div>
            )}

            {geoError && <div className="geo-banner geo-banner--error">{geoError}</div>}

            <small className="form-hint">
              Use auto-detect or enter coordinates manually. You can get coordinates from Google Maps by
              right-clicking a location.
            </small>
          </div>

          <div className="form-group">
            <label>Radius (meters) *</label>
            <input
              type="number"
              value={formData.radiusMeters}
              onChange={(e) => setFormData({ ...formData, radiusMeters: e.target.value })}
              min="10"
              max="10000"
              required
            />
            <small className="form-hint">
              Students must be within this radius to mark attendance (min: 10m, max: 10000m)
            </small>
          </div>

          <div className="form-group">
            <label>Description (optional)</label>
            <textarea
              value={formData.description}
              onChange={(e) => setFormData({ ...formData, description: e.target.value })}
              rows="2"
              placeholder="e.g., Ground floor, Building A"
            />
          </div>

          <div className="form-actions">
            <button type="submit" className="btn btn-primary" disabled={!currentCoords}>
              {editingLocation ? 'Update Location' : 'Create Location'}
            </button>
            <button type="button" className="btn btn-secondary" onClick={() => setShowModal(false)}>
              Cancel
            </button>
          </div>
        </form>
      </Modal>
    </div>
  );
};

export default Locations;
