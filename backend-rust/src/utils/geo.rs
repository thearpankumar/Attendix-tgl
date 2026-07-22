/// Calculate distance between two GPS coordinates in meters using Haversine formula
pub fn calculate_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

    let lat1_rad = lat1.to_radians();
    let lat2_rad = lat2.to_radians();
    let delta_lat = (lat2 - lat1).to_radians();
    let delta_lon = (lon2 - lon1).to_radians();

    let a = (delta_lat / 2.0).sin().powi(2)
        + lat1_rad.cos() * lat2_rad.cos() * (delta_lon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    EARTH_RADIUS_METERS * c
}

/// Check if a point is within a radius of another point
pub fn is_within_radius(
    center_lat: f64,
    center_lon: f64,
    point_lat: f64,
    point_lon: f64,
    radius_meters: f64,
) -> bool {
    calculate_distance(center_lat, center_lon, point_lat, point_lon) <= radius_meters
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_distance() {
        let distance = calculate_distance(40.7128, -74.0060, 34.0522, -118.2437);
        assert!(distance > 3_900_000.0 && distance < 4_000_000.0);
    }

    #[test]
    fn test_is_within_radius() {
        assert!(is_within_radius(12.9716, 77.5946, 12.9720, 77.5950, 1000.0));
        assert!(!is_within_radius(
            12.9716, 77.5946, 13.0000, 78.0000, 1000.0
        ));
    }
}
