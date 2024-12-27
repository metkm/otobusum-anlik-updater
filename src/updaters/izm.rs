use reqwest::header::HeaderMap;
use sqlx::PgPool;
use tracing::info;

use crate::{
    models::izm::{IzmLine, IzmLinesResponse, LoginBody, LoginBodyResponse},
    updater::Updater,
};

#[derive(Debug)]
pub struct IzmUpdater {
    pub client: reqwest::Client,
    pub headers: HeaderMap,
}

impl IzmUpdater {
    pub fn new() -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append(
            "Content-Type",
            "application/json; charset=UTF-8".parse().unwrap(),
        );

        Self {
            client: reqwest::Client::new(),
            headers,
        }
    }
}

impl Updater for IzmUpdater {
    async fn get_credentials(&mut self) -> Result<(), reqwest::Error> {
        let login_body = LoginBody {
            user_name: "tur".to_string(),
            password: "t@r!".to_string(),
        };

        info!("getting login tokens");
        let login_response = self
            .client
            .post("https://appapi.eshot.gov.tr/api/Transportation/Login")
            .headers(self.headers.clone())
            .json(&login_body)
            .send()
            .await?
            .json::<LoginBodyResponse>()
            .await?;

        info!("{:?}", login_response);

        self.headers.insert(
            "Authorization",
            format!("Bearer {}", login_response.data.token)
                .parse()
                .unwrap(),
        );

        info!("getting anonymous user using login token");
        let anonymous_response = self
            .client
            .get("https://appapi.eshot.gov.tr/api/TransportationUser/getAnonymousUser")
            .headers(self.headers.clone())
            .send()
            .await?
            .json::<LoginBodyResponse>()
            .await?;

        self.headers.insert(
            "Authorization",
            format!("Bearer {}", anonymous_response.data.token)
                .parse()
                .unwrap(),
        );

        info!("got tokens");
        Ok(())
    }

    async fn insert_lines(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        info!("getting lines");

        let mut lines: Vec<IzmLine> = Vec::with_capacity(400);
        let mut stop = false;
        let mut offset = 0;

        while !stop {
            info!("getting lines offset {offset}");

            let response = self
                .client
                .get("https://acikveri.bizizmir.com/api/3/action/datastore_search")
                .query(&vec![
                    ("resource_id", "bd6c84f8-49ba-4cf4-81f8-81a0fbb5caa3"),
                    ("offset", &offset.to_string()),
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

        let line_codes: Vec<String> = lines
            .iter()
            .map(|x| x.line_code.to_string())
            .collect::<Vec<String>>();

        let line_titles: Vec<String> = lines
            .iter()
            .map(|x| x.line_name.to_string())
            .collect::<Vec<String>>();

        let line_cities = lines
            .iter()
            .map(|_| "istanbul".to_string())
            .collect::<Vec<String>>();

        let lines_insert_result = sqlx::query!(
            "
                INSERT INTO lines (code, title, city)
                SELECT * FROM UNNEST ($1::text[], $2::text[], $3::text[])
                ON CONFLICT (code, city) DO UPDATE SET
                    code = EXCLUDED.code,
                    city = EXCLUDED.city
            ",
            &line_codes[..],
            &line_titles[..],
            &line_cities[..],
        )
        .execute(db)
        .await?;

        info!("inserted {:?} rows", lines_insert_result.rows_affected());

        Ok(())
    }
}
