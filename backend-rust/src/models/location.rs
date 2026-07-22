use chrono::{DateTime, Utc};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default = "default_radius")]
    pub radius_meters: f64,
    pub description: Option<String>,
    pub created_by: ObjectId,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(with = "bson::serde_helpers::datetime::FromChrono04DateTime")]
    pub created_at: DateTime<Utc>,
}

fn default_radius() -> f64 {
    100.0
}
fn default_true() -> bool {
    true
}

impl Location {
    pub fn collection_name() -> &'static str {
        "locations"
    }

    pub fn validate(&self) -> crate::error::Result<()> {
        if self.latitude < -90.0 || self.latitude > 90.0 {
            return Err(crate::error::AppError::BadRequest(
                "Latitude must be between -90 and 90".into(),
            ));
        }
        if self.longitude < -180.0 || self.longitude > 180.0 {
            return Err(crate::error::AppError::BadRequest(
                "Longitude must be between -180 and 180".into(),
            ));
        }
        if self.radius_meters < 10.0 || self.radius_meters > 10000.0 {
            return Err(crate::error::AppError::BadRequest(
                "Radius must be between 10 and 10000 meters".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationCreate {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub radius_meters: Option<f64>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocationUpdate {
    pub name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub radius_meters: Option<f64>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}
