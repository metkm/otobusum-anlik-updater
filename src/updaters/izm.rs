use sqlx::{PgPool, QueryBuilder};
use tracing::info;

use crate::{
    models::{
        ist::IstTokensResponse,
        izm::{IzmLine, IzmLinesResponse},
    },
    updater::Updater,
};

#[derive(Debug)]
pub struct IzmUpdater {
    pub client: reqwest::Client,
}

impl IzmUpdater {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Updater for IzmUpdater {
    async fn get_credentials(&mut self) -> Result<IstTokensResponse, reqwest::Error> {
        todo!()
    }

    async fn insert_lines(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        info!("getting lines");

        let mut lines: Vec<IzmLine> = Vec::with_capacity(400);
        let mut stop = false;
        let mut offset = 0;

        while !stop {
            info!("getting lines offset {offset}");

            let response = self.client.get("https://acikveri.bizizmir.com/api/3/action/datastore_search")
                .query(&vec![
                    ("resource_id", "bd6c84f8-49ba-4cf4-81f8-81a0fbb5caa3"),
                    ("offset", &offset.to_string())
                ])
                .send()
                .await?
                .json::<IzmLinesResponse>()
                .await?;

            lines.extend(response.result.records);
            offset += 100;

            if offset > response.result.total {
                stop = true;
            }
        }

        let lines_insert_result = QueryBuilder::new("INSERT INTO lines (code, title, city)")
            .push_values(lines, |mut b, record| {
                b.push_bind(record.line_code.to_string());
                b.push_bind(record.line_name);
                b.push_bind("izmir");
            })
            .build()
            .execute(db)
            .await?;

        info!("inserted {:?} rows", lines_insert_result.rows_affected());

        Ok(())
    }
}
