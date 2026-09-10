use serde::{Deserialize, Serialize};
use sqlx::types::chrono::NaiveTime;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DatabaseRoute {
    pub agency_id: Option<i32>,
    pub code: Option<String>,
    pub title: Option<String>,
    pub r#type: Option<i32>,
    pub description: Option<String>,
    pub route_code: Option<String>,
    pub city: String,
}

#[derive(Debug)]
pub struct DatabaseLine {
    pub id: i32,
    pub code: String,
    pub title: String,
    pub city: String,
}

#[derive(Serialize, Deserialize, Debug, sqlx::Type)]
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

pub struct DatabaseLineStop {
    pub line_code: String,
    pub stop_code: i32,
    pub city: String,
    pub route_code: String,
    pub stop_order: i32,
}
