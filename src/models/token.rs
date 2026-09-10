use crate::models::ist::IstTokensResponse;

#[derive(Debug, Clone)]
pub struct Token {
    pub access_token: String,
}

impl From<IstTokensResponse> for Token {
    fn from(value: IstTokensResponse) -> Self {
        Self {
            access_token: value.access_token,
        }
    }
}
