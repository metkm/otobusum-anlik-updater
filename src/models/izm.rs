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
pub struct LoginBody {
    #[serde(rename(serialize = "userName"))]
    pub user_name: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginBodyData {
    #[serde(rename(deserialize = "Item1"))]
    pub token: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginBodyResponse {
    pub data: LoginBodyData,
}
