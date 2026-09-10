use sqlx::PgPool;

use crate::models::token::Token;
use crate::request_client::RequestClient;

pub trait Updater {
    type Item: Updater;

    async fn authorize(&self) -> Result<Token, anyhow::Error>;

    // async fn get_credentials(&mut self) -> Result<(), reqwest::Error>;
    async fn insert_lines(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self::Item>,
    ) -> Result<(), anyhow::Error>;
    async fn insert_routes(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self::Item>,
        offset: usize,
    ) -> Result<(), anyhow::Error>;
    async fn insert_line_stops(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self::Item>,
        offset: usize,
    ) -> Result<(), anyhow::Error>;
    async fn insert_route_paths(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self::Item>,
    ) -> Result<(), anyhow::Error>;
    async fn insert_timetable(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self::Item>,
        offset: usize,
    ) -> Result<(), anyhow::Error>;
}
