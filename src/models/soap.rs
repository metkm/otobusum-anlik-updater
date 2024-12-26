use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct BusLineSoap {
    #[serde(alias = "SHATKODU")]
    pub line_code: String,
    #[serde(alias = "SHATADI")]
    pub line_name: String,
    #[serde(alias = "HAT_UZUNLUGU")]
    pub line_length: f32,
    #[serde(alias = "SEFER_SURESI")]
    pub duration: f32,
}

#[derive(Serialize, Deserialize)]
pub struct BusLineResponseJsonSoap {
    #[serde(alias = "GetHat_jsonResult")]
    pub content: String,
}

#[derive(Serialize, Deserialize)]
pub struct BusLineResponseBodySoap {
    #[serde(alias = "GetHat_jsonResponse")]
    pub content: BusLineResponseJsonSoap,
}

#[derive(Serialize, Deserialize)]
pub struct BusLineResponseSoap {
    #[serde(alias = "Body")]
    pub content: BusLineResponseBodySoap,
}
