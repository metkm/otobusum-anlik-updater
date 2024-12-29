use sqlx::PgPool;

pub trait Updater {
    async fn get_credentials(&mut self) -> Result<(), reqwest::Error>;
    async fn insert_lines(&self, db: &PgPool) -> Result<(), anyhow::Error>;
    async fn insert_line_stops(&self, db: &PgPool) -> Result<(), anyhow::Error>;
}
