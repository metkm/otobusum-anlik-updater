use sqlx::PgPool;

use crate::models::ist::IstTokensResponse;

pub trait Updater {
    async fn get_credentials(&mut self) -> Result<IstTokensResponse, reqwest::Error>;
    async fn insert_lines(&self, db: &PgPool) -> Result<(), anyhow::Error>;
}
