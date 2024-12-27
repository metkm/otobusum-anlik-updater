use serde::{Deserialize, Serialize};
use sqlx::types::chrono::NaiveTime;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseRoute {
    // pub id: i32,
    pub agency_id: Option<i32>,
    pub route_short_name: Option<String>,
    pub route_long_name: Option<String>,
    pub route_type: Option<i32>,
    pub route_desc: Option<String>,
    pub route_code: Option<String>,
    pub city: String,
    // pub route_path: Option<sqlx::types::JsonValue>,
}

pub struct DatabaseLine {
    pub id: i32,
    pub code: String,
    pub title: String,
    pub city: String,
}

#[derive(Serialize, Deserialize, sqlx::Type)]
pub struct LatLng {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Serialize, Default)]
pub struct DatabaseTimetable {
    pub route_long_name: Option<String>,
    pub route_code: String,
    pub city: String,
    pub sunday: Vec<NaiveTime>,
    pub monday: Vec<NaiveTime>,
    pub tuesday: Vec<NaiveTime>,
    pub wednesday: Vec<NaiveTime>,
    pub thursday: Vec<NaiveTime>,
    pub friday: Vec<NaiveTime>,
    pub saturday: Vec<NaiveTime>,
}
