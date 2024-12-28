use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct IzmLine {
    #[serde(alias = "HAT_NO")]
    pub line_code: u32,
    #[serde(alias = "HAT_ADI")]
    pub line_name: String,
    #[serde(alias = "HAT_BASLANGIC")]
    pub line_start: String,
    #[serde(alias = "HAT_BITIS")]
    pub line_end: String,
}

#[derive(Serialize, Deserialize)]
pub struct IzmLinesResult {
    pub records: Vec<IzmLine>,
    pub total: u32,
}

#[derive(Serialize, Deserialize)]
pub struct IzmLinesResponse {
    pub result: IzmLinesResult,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IzmLoginBody {
    #[serde(rename(serialize = "userName"))]
    pub user_name: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IzmLoginBodyData {
    #[serde(rename(deserialize = "Item1"))]
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct IzmLoginBodyResponse {
    pub data: IzmLoginBodyData,
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub struct IzmSearchResult {
    pub id: i32,
    pub name: String,
    pub code: String,
}

#[derive(Serialize, Deserialize)]
pub struct IzmSearchResponse {
    pub data: Vec<IzmSearchResult>,
}

// Eshot line response and stuff from here
#[derive(Serialize, Deserialize)]
pub struct EshotLineResponse {
    pub data: Vec<EShotLineData>,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Direction {
    G = 1,
    D = 2,
    R = 3, // Ring hat
}

impl TryFrom<i32> for Direction {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Direction::G),
            2 => Ok(Direction::D),
            3 => Ok(Direction::R),
            _ => Err(()),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct EshotLineStation {
    pub lat: f64,
    pub lng: f64,
    pub id: i32,
    pub name: String,
    pub code: String,
}

#[derive(Serialize, Deserialize)]
pub struct EshotTimetable {
    pub time: String,
    pub day: i32,
}

#[derive(Serialize, Deserialize)]
pub struct EShotLineData {
    // #[serde(alias = "lineId")]
    // pub line_id: i32,
    // pub starting: String,
    // pub ending: String,
    pub direction: i32,
    pub tracks: Vec<String>,
    pub stations: Vec<EshotLineStation>,
    pub times: Vec<EshotTimetable>,
    // pub id: i32,
}
