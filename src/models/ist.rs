use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct IstTokensResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub expire_date: u64,
}

#[derive(Deserialize)]
pub struct IstLineRoutesResponse {
    // #[serde(alias = "HAT_HAT_ADI")]
    // pub line_name: String,
    #[serde(alias = "HAT_HAT_KODU")]
    pub line_code: String,
    // #[serde(alias = "HAT_ID")]
    // pub line_id: u32,
    // #[serde(alias = "GUZERGAH_DEPAR_NO")]
    // pub route_departure_no: u32,
    #[serde(alias = "GUZERGAH_GUZERGAH_ADI")]
    pub route_name: String,
    #[serde(alias = "GUZERGAH_GUZERGAH_KODU")]
    pub route_code: String,
    // #[serde(alias = "GUZERGAH_ID")]
    // pub route_id: u32,
    // #[serde(alias = "GUZERGAH_YON")]
    // pub route_direction: u32,
}

#[derive(Deserialize)]
pub struct StopGeoLocation {
    pub x: f64,
    pub y: f64,
}

#[derive(Deserialize)]
pub struct IstLineStopsResponse {
    #[serde(alias = "GUZERGAH_GUZERGAH_KODU")]
    pub route_code: String,
    #[serde(alias = "GUZERGAH_SEGMENT_SIRA")]
    pub stop_order: i32,
    #[serde(alias = "DURAK_ADI")]
    pub stop_name: String,
    #[serde(alias = "DURAK_DURAK_KODU")]
    pub stop_code: i32,
    #[serde(alias = "DURAK_GEOLOC")]
    pub stop_geo: StopGeoLocation,
    #[serde(alias = "ILCELER_ILCEADI")]
    pub province: String,
}

impl PartialEq for IstLineStopsResponse {
    fn eq(&self, other: &Self) -> bool {
        self.stop_code == other.stop_code
    }

    fn ne(&self, other: &Self) -> bool {
        self.stop_code != other.stop_code
    }
}

#[derive(Deserialize)]
pub struct IstRoutePathResponse {
    pub line: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum DayType {
    I,
    C,
    P,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct IstTimetableResponse {
    #[serde(alias = "K_ORER_SGUZERGAH")]
    pub route_code: String,
    #[serde(alias = "K_ORER_DTSAATGIDIS")]
    pub time: String,
    #[serde(alias = "K_ORER_SGUNTIPI")]
    pub day_type: DayType,
}
