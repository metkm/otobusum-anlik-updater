use std::{
    collections::{HashMap, HashSet},
    fs::{File, create_dir},
    io::{Read, Write},
    path::Path,
};

use chrono::NaiveDateTime;
use reqwest::header::HeaderMap;
use sqlx::{types::Json, PgPool, QueryBuilder};
use tracing::{info, warn};

use crate::{
    models::{
        database::{DatabaseLine, DatabaseRoute, DatabaseTimetable, LatLng},
        ist::{
            DayType, IstLineRoutesResponse, IstLineStopsResponse, IstRoutePathGeoJson, IstRoutePathGeoJsonFeature, IstTimetableResponse, IstTokensResponse
        },
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

            info!("sleeping for 5 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
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
                info!("getting route stops for {}", &line.code);
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

                if stops.len() < 1 {
                    warn!("no stops found for {}. skipping", &line.code);
                    continue;
                }

                let insert_line_stops_result = QueryBuilder::new(
                    "INSERT INTO line_stops (line_code, stop_code, stop_order, city, route_code)",
                )
                .push_values(&stops, |mut b, record| {
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
                    "inserted {} line stops for {}",
                    insert_line_stops_result.rows_affected(),
                    &line.code
                );

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
                    "inserted/updated {} stops for {}",
                    insert_stops_result.rows_affected(),
                    &line.code
                );
            }

            info!("sleeping for 10 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }

        Ok(())
    }

    async fn insert_route_paths(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        let routes = sqlx::query_as!(
            DatabaseRoute,
            "SELECT
                agency_id,
                route_short_name,
                route_long_name,
                route_type,
                route_desc,
                route_code,
                city
            FROM
                routes
            "
        )
        .fetch_all(db)
        .await?;

        let file_path = Path::new("./data/path.geojson");
        create_dir(Path::new("./data")).ok();

        let geojson: IstRoutePathGeoJson = {
            if !Path::exists(&file_path) {
                info!("downloading geojson file because It's not found");

                let response = self.client
                    .get("https://data.ibb.gov.tr/dataset/b48d2095-851c-413c-8d36-87d2310a22b5/resource/4ccb4d29-c2b6-414a-b324-d2c9962b18e2/download/iett-hat-guzergahlar.geojson")
                    .send()
                    .await?;

                let response_body = response.bytes().await?;

                let mut out = File::create("./data/path.geojson")?;
                out.write(&response_body)?;

                serde_json::from_slice(&response_body.slice(..))?
            } else {
                info!("parsing geojson file");

                let mut file = File::open(&file_path)?;
                let mut buffer = String::with_capacity(1_000_000);

                file.read_to_string(&mut buffer)?;
                serde_json::from_str(&buffer)?
            }
        };

        let database_route_codes: Vec<String> = routes
            .into_iter()
            .filter_map(|rout| rout.route_code)
            .collect();

        let filtered_routes = geojson.features
            .into_iter()
            .filter(|feat| !database_route_codes.contains(&feat.properties.route_code))
            .collect::<Vec<IstRoutePathGeoJsonFeature>>();

        let inserted_route_paths_result = QueryBuilder::new(
            "INSERT INTO route_paths (route_code, route_path, city)"
        )
            .push_values(filtered_routes, |mut b, record| {
                let coords = record.geometry.coordinates
                    .into_iter()
                    .flatten()
                    .map(|coord| LatLng {
                        lng: *coord.get(0).unwrap(),
                        lat: *coord.get(1).unwrap(),
                    })
                    .collect::<Vec<LatLng>>();

                b.push_bind(record.properties.route_code)
                    .push_bind(Json(coords))
                    .push_bind("istanbul");

            })
            .push("ON CONFLICT (route_code, city) DO UPDATE SET
                         route_path=EXCLUDED.route_path
            ")
            .build()
            .execute(db)
            .await?;

        info!("inserted/updated {} route paths", inserted_route_paths_result.rows_affected());

        Ok(())
    }

    async fn insert_timetable(&self, db: &PgPool) -> Result<(), anyhow::Error> {
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

        info!("got {} lines for timetable function", lines.len());

        for line in lines {
            let timetable_body = &serde_json::json!({
                "alias": "akyolbilGetTimeTable",
                "data": {
                    "HATYONETIM.GUZERGAH.HAT_KODU": &line.code
                }
            });

            info!("getting timetable for {}", &line.code);
            let timetable_response = self
                .client
                .post("https://ntcapi.iett.istanbul/service")
                .body(timetable_body.to_string())
                .headers(self.headers.clone())
                .send()
                .await?
                .json::<Vec<IstTimetableResponse>>()
                .await?;

            let mut timetables_grouped: HashMap<String, Vec<IstTimetableResponse>> = HashMap::new();
            for timetable in timetable_response {
                if let Some(tables) = timetables_grouped.get_mut(&timetable.route_code) {
                    tables.push(timetable);
                } else {
                    timetables_grouped.insert(timetable.route_code.clone(), vec![timetable]);
                }
            }

            for (route_code, timetables) in timetables_grouped {
                let mut timetable_to_insert = DatabaseTimetable {
                    city: "istanbul".to_string(),
                    route_code,
                    ..Default::default()
                };

                for timetable in timetables {
                    let time = NaiveDateTime::parse_from_str(&timetable.time, "%Y-%m-%d %H:%M:%S")
                        .unwrap()
                        .time();

                    if timetable.day_type == DayType::I {
                        timetable_to_insert.monday.push(time);
                        timetable_to_insert.tuesday.push(time);
                        timetable_to_insert.wednesday.push(time);
                        timetable_to_insert.thursday.push(time);
                        timetable_to_insert.friday.push(time);
                    } else if timetable.day_type == DayType::C {
                        timetable_to_insert.saturday.push(time);
                    } else if timetable.day_type == DayType::P {
                        timetable_to_insert.sunday.push(time);
                    }
                }

                let inserted_timetable = sqlx::query!("
                    INSERT INTO timetable (route_code, city, sunday, monday, tuesday, wednesday, thursday, friday, saturday)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                    ON CONFLICT (route_code, city) DO UPDATE SET
                        sunday=EXCLUDED.sunday,
                        monday=EXCLUDED.monday,
                        tuesday=EXCLUDED.tuesday,
                        wednesday=EXCLUDED.wednesday,
                        thursday=EXCLUDED.thursday,
                        friday=EXCLUDED.friday,
                        saturday=EXCLUDED.saturday
                    ",
                    timetable_to_insert.route_code,
                    timetable_to_insert.city,
                    &timetable_to_insert.sunday,
                    &timetable_to_insert.monday,
                    &timetable_to_insert.tuesday,
                    &timetable_to_insert.wednesday,
                    &timetable_to_insert.thursday,
                    &timetable_to_insert.friday,
                    &timetable_to_insert.saturday
                )
                    .execute(db)
                    .await?;

                info!(
                    "inserted {} timetable rows for {}",
                    inserted_timetable.rows_affected(),
                    &line.code
                );
            }

            info!("sleeping for 10 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }

        Ok(())
    }
}
