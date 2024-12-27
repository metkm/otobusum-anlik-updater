use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct IzmLine {
    #[serde(alias = "HAT_NO")]
    pub line_code: u32,
    #[serde(alias = "HAT_ADI")]
    pub line_name: String,
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
