use std::collections::{HashMap, HashSet};

use chrono::NaiveDateTime;
use regex::Regex;
use reqwest::header::HeaderMap;
use sqlx::{PgPool, QueryBuilder};
use tracing::{info, warn};

use crate::{
    constants::SLEEP_DURATION,
    models::{
        database::{DatabaseLine, DatabaseRoute, DatabaseTimetable, LatLng},
        ist::{
            DayType, IstLineRoutesResponse, IstLineStopsResponse, IstRoutePathResponse,
            IstTimetableResponse, IstTokensResponse,
        },
        soap::{BusLineResponseSoap, BusLineSoap},
        token::Token,
        updater::Updater,
    },
    request_client::RequestClient,
};

#[derive(Debug)]
pub struct IstUpdater {
    // pub client: reqwest::Client,
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
            // client: reqwest::Client::new(),
            headers,
        }
    }
}

impl Updater for IstUpdater {
    type Item = Self;

    async fn authorize(&self) -> Result<Token, anyhow::Error> {
        let client = reqwest::Client::new();

        let mut body = HashMap::new();
        body.insert("client_id", std::env::var("IBB_CLIENT_ID").unwrap());
        body.insert("client_secret", std::env::var("IBB_CLIENT_SECRET").unwrap());
        body.insert("grant_type", "client_credentials".to_string());
        body.insert("scope", std::env::var("IBB_CLIENT_SCOPE").unwrap());

        let response: IstTokensResponse = client
            .post("https://ntcapi.iett.istanbul/oauth2/v2/auth")
            .headers(self.headers.clone())
            .json(&body)
            .send()
            .await?
            .json()
            .await?;

        Ok(response.into())
    }

    async fn insert_lines(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self>,
    ) -> Result<(), anyhow::Error> {
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
        let response = rq
            .request(|http, _| {
                http.post("https://api.ibb.gov.tr/iett/UlasimAnaVeri/HatDurakGuzergah.asmx")
                    .header("Content-Type", "text/xml; charset=UTF-8")
                    .header("SOAPAction", r#""http://tempuri.org/GetHat_json""#)
                    .body(body)
            })
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

    async fn insert_routes(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self>,
        offset: usize,
    ) -> Result<(), anyhow::Error> {
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

        for (index, line) in lines.iter().skip(offset).enumerate() {
            for direction in &[119, 120] {
                let routes_body = &serde_json::json!({
                    "alias": "mainGetLine_basic",
                    "data": {
                        "HATYONETIM.GUZERGAH.YON": direction,
                        "HATYONETIM.HAT.HAT_KODU": &line.code
                    }
                });

                info!(
                    "{}: getting line routes for {}, direction {}",
                    index, &line.code, direction
                );
                let line_routes = rq
                    .request(|http, _| {
                        http.post("https://ntcapi.iett.istanbul/service")
                            .body(routes_body.to_string())
                            .headers(self.headers.clone())
                    })
                    .await?
                    .json::<Vec<IstLineRoutesResponse>>()
                    .await?;

                if line_routes.is_empty() {
                    info!("skipping {}, routes vec is empty", &line.code);
                    continue;
                }

                let routes_insert_result = QueryBuilder::new(
                    "INSERT INTO routes (agency_id, code, title, type, route_code, city)",
                )
                .push_values(line_routes, |mut b, record| {
                    b.push_bind(1)
                        .push_bind(record.line_code)
                        .push_bind(
                            record
                                .route_name
                                .unwrap_or(record.line_name.to_string())
                                .trim()
                                .to_string(),
                        )
                        .push_bind(3)
                        .push_bind(record.route_code)
                        .push_bind("istanbul");
                })
                .push(
                    "
                    ON CONFLICT (route_code, city) DO UPDATE SET
                        agency_id=EXCLUDED.agency_id,
                        code=EXCLUDED.code,
                        title=EXCLUDED.title,
                        type=EXCLUDED.type,
                        route_code=EXCLUDED.route_code
                ",
                )
                .build()
                .execute(db)
                .await?;

                info!(
                    "{}: inserted/updated {} route rows",
                    index,
                    routes_insert_result.rows_affected()
                );
            }

            info!("sleeping for {} seconds", SLEEP_DURATION.as_secs());
            tokio::time::sleep(SLEEP_DURATION).await;
        }

        Ok(())
    }

    async fn insert_line_stops(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self>,
        offset: usize,
    ) -> Result<(), anyhow::Error> {
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

        for (index, line) in lines.iter().skip(offset).enumerate() {
            for direction in &[119, 120] {
                info!("{}: getting route stops for {}", index, &line.code);

                let stops_body = &serde_json::json!({
                    "alias": "mainGetRoute",
                    "data": {
                        "HATYONETIM.GUZERGAH.YON": direction,
                        "HATYONETIM.HAT.HAT_KODU": &line.code
                    }
                });

                let route_stops = rq
                    .request(|http, _| {
                        http.post("https://ntcapi.iett.istanbul/service")
                            .body(stops_body.to_string())
                            .headers(self.headers.clone())
                    })
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

                if stops.is_empty() {
                    warn!("{}:no stops found for {}. skipping", index, &line.code);
                    continue;
                }

                let insert_line_stops_result = QueryBuilder::new(
                    "INSERT INTO line_stops (line_code, stop_code, stop_order, city, route_code)",
                )
                .push_values(&stops, |mut b, record| {
                    b.push_bind(&line.code)
                        .push_bind(record.stop_code)
                        .push_bind(record.order)
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
                    "{}: inserted {} line stops for {}",
                    index,
                    insert_line_stops_result.rows_affected(),
                    &line.code
                );

                let insert_stops_result = QueryBuilder::new(
                    "INSERT INTO stops (stop_code, name, lng, lat, province, city)",
                )
                .push_values(&stops, |mut b, record| {
                    b.push_bind(record.stop_code)
                        .push_bind(&record.name)
                        .push_bind(record.stop_geo.x)
                        .push_bind(record.stop_geo.y)
                        .push_bind(&record.province)
                        .push_bind("istanbul");
                })
                .push(
                    "
                    ON CONFLICT (stop_code, city) DO UPDATE SET
                        name=EXCLUDED.name,
                        lng=EXCLUDED.lng,
                        lat=EXCLUDED.lat
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

            info!("sleeping for {} seconds", SLEEP_DURATION.as_secs());
            tokio::time::sleep(SLEEP_DURATION).await;
        }

        Ok(())
    }

    async fn insert_route_paths(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self>,
    ) -> Result<(), anyhow::Error> {
        let re = Regex::new(r#"(\d+(?:\.\d+)?)\s+(\d+(?:\.\d+)?)"#).unwrap();

        let routes = sqlx::query_as!(
            DatabaseRoute,
            "SELECT
                agency_id,
                code,
                title,
                type,
                description,
                route_code,
                city
            FROM
                routes
            "
        )
        .fetch_all(db)
        .await?;

        // let lines = sqlx::query_as!(
        //     DatabaseLine,
        //     r#"
        //         SELECT
        //             *
        //         FROM
        //             lines
        //         WHERE
        //             city = 'istanbul'
        //         ORDER BY
        //             code
        //     "#
        // )
        // .fetch_all(db)
        // .await?;

        for (index, route) in routes.iter().enumerate() {
            let route_code = route.route_code.as_ref().unwrap();
            info!("{}: getting route path for {}", index, route_code);

            let response = rq
                .request(|http, _| {
                    http.get(format!(
                        "https://iett.istanbul/tr/RouteStation/GetRoutePinV2?q={}",
                        route_code
                    ))
                })
                .await?
                .json::<Vec<IstRoutePathResponse>>()
                .await?;

            let route_paths = response.get(0);

            let Some(route_path) = route_paths else {
                warn!(
                    "skipping route path for {} because it's returned request array is empty.",
                    route_code
                );
                continue;
            };

            let lat_lng_list = re
                .captures_iter(&route_path.line)
                .map(|x| {
                    let numbers: Vec<&str> = x.get_match().as_str().split_whitespace().collect();

                    let lng = numbers[0].parse::<f64>().unwrap_or(0.0);
                    let lat = numbers[1].parse::<f64>().unwrap_or(0.0);

                    LatLng { lng, lat }
                })
                .collect::<Vec<LatLng>>();

            let inserted_route_paths_result = sqlx::query!(
                r#"
                        INSERT INTO route_paths (route_code, path, city)
                        VALUES ($1, $2, $3)
                        ON CONFLICT (route_code, city)
                        DO UPDATE SET path = EXCLUDED.path
                    "#,
                route_code,
                serde_json::to_value(&lat_lng_list)?,
                "istanbul",
            )
            .execute(db)
            .await?;

            info!(
                "inserted/updated {} route paths",
                inserted_route_paths_result.rows_affected()
            );


            info!("sleeping for {} seconds", SLEEP_DURATION.as_secs());
            tokio::time::sleep(SLEEP_DURATION).await;
        }

        // let routes = sqlx::query_as!(
        //     DatabaseRoute,
        //     "SELECT
        //         agency_id,
        //         code,
        //         title,
        //         type,
        //         description,
        //         route_code,
        //         city
        //     FROM
        //         routes
        //     "
        // )
        // .fetch_all(db)
        // .await?;

        // let file_path = Path::new("./data/path.geojson");
        // create_dir(Path::new("./data")).ok();

        // let geojson: IstRoutePathGeoJson = {
        //     if !Path::exists(file_path) {
        //         info!("downloading geojson file because It's not found");

        //         let response = rq
        //             .request(|http, _| {
        //                 http.get("https://data.ibb.gov.tr/dataset/iett-hat-guzergahlari/resource/4ccb4d29-c2b6-414a-b324-d2c9962b18e2/geojson_download")
        //             }).await?;

        //         let response_body = response.bytes().await?;

        //         let mut out = File::create("./data/path.geojson")?;
        //         out.write_all(&response_body)?;

        //         serde_json::from_slice(&response_body.slice(..))?
        //     } else {
        //         info!("parsing geojson file");

        //         let mut file = File::open(file_path)?;
        //         let mut buffer = String::with_capacity(1_000_000);

        //         file.read_to_string(&mut buffer)?;
        //         serde_json::from_str(&buffer)?
        //     }
        // };

        // let database_route_codes: Vec<String> = routes
        //     .into_iter()
        //     .filter_map(|rout| rout.route_code)
        //     .collect();

        // let filtered_routes = geojson
        //     .features
        //     .into_iter()
        //     .filter(|feat| database_route_codes.contains(&feat.properties.route_code))
        //     .collect::<Vec<IstRoutePathGeoJsonFeature>>();

        // let inserted_route_paths_result =
        //     QueryBuilder::new("INSERT INTO route_paths (route_code, path, city)")
        //         .push_values(filtered_routes, |mut b, record| {
        //             let coords = record
        //                 .geometry
        //                 .coordinates
        //                 .into_iter()
        //                 .map(|coord| LatLng {
        //                     lng: *coord.first().unwrap(),
        //                     lat: *coord.get(1).unwrap(),
        //                 })
        //                 .collect::<Vec<LatLng>>();

        //             b.push_bind(record.properties.route_code)
        //                 .push_bind(Json(coords))
        //                 .push_bind("istanbul");
        //         })
        //         .push(
        //             "ON CONFLICT (route_code, city) DO UPDATE SET
        //                  path=EXCLUDED.path
        //     ",
        //         )
        //         .build()
        //         .execute(db)
        //         .await?;

        // info!(
        //     "inserted/updated {} route paths",
        //     inserted_route_paths_result.rows_affected()
        // );

        Ok(())
    }

    async fn insert_timetable(
        &self,
        db: &PgPool,
        rq: &RequestClient<Self>,
        offset: usize,
    ) -> Result<(), anyhow::Error> {
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

        for (index, line) in lines.iter().skip(offset).enumerate() {
            let timetable_body = &serde_json::json!({
                "alias": "akyolbilGetTimeTable",
                "data": {
                    "HATYONETIM.GUZERGAH.HAT_KODU": &line.code
                }
            });

            info!("{}: getting timetable for {}", index, &line.code);
            let timetable_response = rq
                .request(|http, _| {
                    http.post("https://ntcapi.iett.istanbul/service")
                        .body(timetable_body.to_string())
                        .headers(self.headers.clone())
                })
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
                    "{}: inserted {} timetable rows for {}",
                    index,
                    inserted_timetable.rows_affected(),
                    &line.code
                );
            }

            info!("sleeping for {} seconds", SLEEP_DURATION.as_secs());
            tokio::time::sleep(SLEEP_DURATION).await;
        }

        Ok(())
    }
}
