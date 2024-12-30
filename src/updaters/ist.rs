use std::collections::{HashMap, HashSet};

use reqwest::header::HeaderMap;
use sqlx::{PgPool, QueryBuilder};
use tracing::info;

use crate::{
    models::{
        database::DatabaseLine,
        ist::{IstLineRoutesResponse, IstLineStopsResponse, IstTokensResponse},
        soap::{BusLineResponseSoap, BusLineSoap},
    },
    updater::Updater,
};

#[derive(Debug)]
pub struct IstUpdater {
    pub client: reqwest::Client,
    pub headers: HeaderMap,
}

impl IstUpdater {
    pub fn new() -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.append("Host", "ntcapi.iett.istanbul".parse().unwrap());
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

impl Updater for IstUpdater {
    async fn get_credentials(&mut self) -> Result<(), reqwest::Error> {
        let mut body = HashMap::new();
        body.insert("client_id", std::env::var("IBB_CLIENT_ID").unwrap());
        body.insert("client_secret", std::env::var("IBB_CLIENT_SECRET").unwrap());
        body.insert("grant_type", "client_credentials".to_string());
        body.insert("scope", std::env::var("IBB_CLIENT_SCOPE").unwrap());

        let response: IstTokensResponse = self
            .client
            .post("https://ntcapi.iett.istanbul/oauth2/v2/auth")
            .headers(self.headers.clone())
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        self.headers.insert(
            "Authorization",
            format!("Bearer {}", response.access_token).parse().unwrap(),
        );

        info!("got tokens");
        Ok(())
    }

    async fn insert_lines(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        let body = r#"
        <soap:Envelope
            xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
                <soap:Body>
                    <GetHat_json
                        xmlns="http://tempuri.org/">
                        <HatKodu></HatKodu>
                    </GetHat_json>
                </soap:Body>
            </soap:Envelope>
        "#;

        info!("getting lines");
        let response = self
            .client
            .post("https://api.ibb.gov.tr/iett/UlasimAnaVeri/HatDurakGuzergah.asmx")
            .header("Content-Type", "text/xml; charset=UTF-8")
            .header("SOAPAction", r#""http://tempuri.org/GetHat_json""#)
            .body(body)
            .send()
            .await?;

        let text = response.text().await?;

        info!("parsing lines");
        let parsed = serde_xml_rs::from_str::<BusLineResponseSoap>(&text)?;
        let bus_lines = serde_json::from_str::<Vec<BusLineSoap>>(&parsed.content.content.content)?;

        let lines_insert_result = QueryBuilder::new("INSERT INTO lines (code, title, city)")
            .push_values(bus_lines, |mut b, new_line| {
                b.push_bind(new_line.line_code);
                b.push_bind(new_line.line_name);
                b.push_bind("istanbul");
            })
            .push(
                "ON CONFLICT (code, city) DO UPDATE SET
                    title = EXCLUDED.title
            ",
            )
            .build()
            .execute(db)
            .await?;

        info!("inserted {:?} rows", lines_insert_result.rows_affected());

        Ok(())
    }

    async fn insert_routes(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        let lines = sqlx::query_as!(
            DatabaseLine,
            r#"
                SELECT
                    *
                FROM
                    lines
                WHERE
                    city = 'istanbul'
                ORDER BY
                    code
            "#
        )
        .fetch_all(db)
        .await?;

        for line in lines {
            for direction in &[119, 120] {
                let routes_body = &serde_json::json!({
                    "alias": "mainGetLine_basic",
                    "data": {
                        "HATYONETIM.GUZERGAH.YON": direction,
                        "HATYONETIM.HAT.HAT_KODU": &line.code
                    }
                });

                info!(
                    "getting line routes for {}, direction {}",
                    &line.code, direction
                );
                let line_routes = self
                    .client
                    .post("https://ntcapi.iett.istanbul/service")
                    .body(routes_body.to_string())
                    .headers(self.headers.clone())
                    .send()
                    .await?
                    .json::<Vec<IstLineRoutesResponse>>()
                    .await?;

                let routes_insert_result = QueryBuilder::new(
                    "INSERT INTO routes (agency_id, route_short_name, route_long_name, route_type, route_code, city)"
                )
                .push_values(line_routes, |mut b, record| {
                    b.push_bind(1)
                    .push_bind(record.line_code)
                    .push_bind(record.route_name.trim().to_string())
                    .push_bind(3)
                    .push_bind(record.route_code)
                    .push_bind("istanbul");
                })
                .push("
                    ON CONFLICT (route_code, city) DO UPDATE SET
                        agency_id=EXCLUDED.agency_id,
                        route_short_name=EXCLUDED.route_short_name,
                        route_long_name=EXCLUDED.route_long_name,
                        route_type=EXCLUDED.route_type,
                        route_code=EXCLUDED.route_code
                ")
                .build()
                .execute(db)
                .await?;

                info!(
                    "inserted/updated {} route rows",
                    routes_insert_result.rows_affected()
                );
            }
        }

        Ok(())
    }

    async fn insert_line_stops(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        let lines = sqlx::query_as!(
            DatabaseLine,
            r#"
                SELECT
                    *
                FROM
                    lines
                WHERE
                    city = 'istanbul'
                ORDER BY
                    code
            "#
        )
        .fetch_all(db)
        .await?;

        info!("found {} lines", lines.len());

        for line in lines {
            for direction in &[119, 120] {
                info!("getting route stops");
                let stops_body = &serde_json::json!({
                    "alias": "mainGetRoute",
                    "data": {
                        "HATYONETIM.GUZERGAH.YON": direction,
                        "HATYONETIM.HAT.HAT_KODU": &line.code
                    }
                });

                let route_stops = self
                    .client
                    .post("https://ntcapi.iett.istanbul/service")
                    .body(stops_body.to_string())
                    .headers(self.headers.clone())
                    .send()
                    .await?
                    .json::<Vec<IstLineStopsResponse>>()
                    .await?;

                let insert_line_stops_result = QueryBuilder::new(
                    "INSERT INTO line_stops (line_code, stop_code, stop_order, city, route_code)",
                )
                .push_values(&route_stops, |mut b, record| {
                    b.push_bind(&line.code)
                        .push_bind(&record.stop_code)
                        .push_bind(&record.stop_order)
                        .push_bind("istanbul")
                        .push_bind(&record.route_code);
                })
                .push(
                    "ON CONFLICT (route_code, stop_code, city)
                    DO UPDATE SET
                        stop_order=EXCLUDED.stop_order
                ",
                )
                .build()
                .execute(db)
                .await?;

                info!(
                    "inserted {} line stops",
                    insert_line_stops_result.rows_affected()
                );

                let mut stop_codes: HashSet<i32> = HashSet::new();
                let stops: Vec<&IstLineStopsResponse> = route_stops
                    .iter()
                    .filter_map(|x| {
                        if stop_codes.contains(&x.stop_code) {
                            None
                        } else {
                            stop_codes.insert(x.stop_code);
                            Some(x)
                        }
                    })
                    .collect();

                let insert_stops_result = QueryBuilder::new(
                    "INSERT INTO stops (stop_code, stop_name, x_coord, y_coord, province, city)",
                )
                .push_values(&stops, |mut b, record| {
                    b.push_bind(record.stop_code)
                        .push_bind(&record.stop_name)
                        .push_bind(record.stop_geo.x)
                        .push_bind(record.stop_geo.y)
                        .push_bind(&record.province)
                        .push_bind("istanbul");
                })
                .push(
                    "
                    ON CONFLICT (stop_code, city) DO UPDATE SET
                        stop_name=EXCLUDED.stop_name,
                        x_coord=EXCLUDED.x_coord,
                        y_coord=EXCLUDED.y_coord
                ",
                )
                .build()
                .execute(db)
                .await?;

                info!(
                    "inserted/updated {} stops",
                    insert_stops_result.rows_affected()
                );
            }

            info!("sleeping for 5 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }

        Ok(())
    }
}
