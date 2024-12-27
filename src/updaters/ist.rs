use std::collections::HashMap;

use reqwest::header::HeaderMap;
use sqlx::{PgPool, QueryBuilder};
use tracing::info;

use crate::{
    models::{
        ist::IstTokensResponse,
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
            .build()
            .execute(db)
            .await?;

        info!("inserted {:?} rows", lines_insert_result.rows_affected());

        Ok(())
    }
}
