use reqwest::header::HeaderMap;
use sqlx::{PgPool, QueryBuilder};
use tracing::info;

use crate::{
    models::{database::DatabaseRoute, izm::{IzmLine, IzmLinesResponse, LoginBody, LoginBodyResponse}},
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

        let lines_insert_result = QueryBuilder::new("INSERT INTO lines (code, title, city)")
            .push_values(&lines, |mut b, record| {
                b.push_bind(record.line_code.to_string());
                b.push_bind(record.line_name.clone());
                b.push_bind("izmir");
            })
            .push(
                "ON CONFLICT (code, city) DO UPDATE SET
                    code = EXCLUDED.code,
                    city = EXCLUDED.city
            ")
            .build()
            .execute(db)
            .await?;

        info!(
            "inserted/updated {:?} rows",
            lines_insert_result.rows_affected()
        );
        info!("also creating default routes for every line");

        let route_codes = lines.iter()
            .map(|line| {
                [
                    DatabaseRoute {
                        agency_id: Some(1),
                        route_short_name: Some(line.line_code.to_string()),
                        route_long_name: Some(format!("{} - {}", line.line_start, line.line_end)),
                        route_type: Some(3),
                        route_code: Some(format!("{}_G_D0", line.line_code)),
                        route_desc: None,
                        city: "izmir".to_string()
                    },
                    DatabaseRoute {
                        agency_id: Some(1),
                        route_short_name: Some(line.line_code.to_string()),
                        route_long_name: Some(format!("{} - {}", line.line_start, line.line_end)),
                        route_type: Some(3),
                        route_code: Some(format!("{}_D_D0", line.line_code)),
                        route_desc: None,
                        city: "izmir".to_string()
                    },
                ]
            })
            .flatten()
            .collect::<Vec<DatabaseRoute>>();
        
        let routes_insert_result = QueryBuilder::new("INSERT INTO routes (agency_id, route_short_name, route_long_name, route_type, route_desc, route_code, city)")
            .push_values(route_codes, |mut b, record| {
                b.push_bind(record.agency_id);
                b.push_bind(record.route_short_name);
                b.push_bind(record.route_long_name);
                b.push_bind(record.route_type);
                b.push_bind(record.route_desc);
                b.push_bind(record.route_code);
                b.push_bind("izmir");
            })
            .push("
                ON CONFLICT (route_code, city) DO UPDATE SET
                    agency_id=EXCLUDED.agency_id,
                    route_short_name=EXCLUDED.route_short_name,
                    route_long_name=EXCLUDED.route_long_name,
                    route_type=EXCLUDED.route_type,
                    route_desc=EXCLUDED.route_desc,
                    route_code=EXCLUDED.route_code
            ")
            .build()
            .execute(db)
            .await?;

        info!("inserted/updated {} route rows", routes_insert_result.rows_affected());

        Ok(())
    }
}
