use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct IstTokensResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub refresh_token: String,
    pub expire_date: u64,
}
