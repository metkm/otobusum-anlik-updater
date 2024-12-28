use std::{collections::HashSet, str::FromStr};

use chrono::NaiveTime;
use reqwest::header::HeaderMap;
use sqlx::{PgPool, QueryBuilder, types::Json};
use tracing::{info, warn};

use crate::{
    models::{
        database::{DatabaseLine, DatabaseRoute, DatabaseTimetable, LatLng},
        izm::{
            Direction, EshotLineResponse, IzmLine, IzmLinesResponse, IzmLoginBody,
            IzmLoginBodyResponse, IzmSearchResponse, IzmSearchResult,
        },
    },
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
        let login_body = IzmLoginBody {
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
            .json::<IzmLoginBodyResponse>()
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
            .json::<IzmLoginBodyResponse>()
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
            ",
            )
            .build()
            .execute(db)
            .await?;

        info!(
            "inserted/updated {:?} rows",
            lines_insert_result.rows_affected()
        );
        info!("also creating default routes for every line");

        let route_codes = lines
            .iter()
            .map(|line| {
                [
                    DatabaseRoute {
                        agency_id: Some(1),
                        route_short_name: Some(line.line_code.to_string()),
                        route_long_name: Some(format!("{} - {}", line.line_start, line.line_end)),
                        route_type: Some(3),
                        route_code: Some(format!("{}_G_D0", line.line_code)),
                        route_desc: None,
                        city: "izmir".to_string(),
                    },
                    DatabaseRoute {
                        agency_id: Some(1),
                        route_short_name: Some(line.line_code.to_string()),
                        route_long_name: Some(format!("{} - {}", line.line_start, line.line_end)),
                        route_type: Some(3),
                        route_code: Some(format!("{}_D_D0", line.line_code)),
                        route_desc: None,
                        city: "izmir".to_string(),
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

        info!(
            "inserted/updated {} route rows",
            routes_insert_result.rows_affected()
        );

        Ok(())
    }

    async fn insert_line_stops(&self, db: &PgPool) -> Result<(), anyhow::Error> {
        info!("getting lines");

        let lines = sqlx::query_as!(DatabaseLine, "SELECT * FROM lines WHERE city = 'izmir'")
            .fetch_all(db)
            .await?;

        let mut search_cache: HashSet<IzmSearchResult> = HashSet::new();

        for line in lines {
            let found_in_cache = search_cache.iter().find(|res| res.code == line.code);

            let search_result = match found_in_cache {
                Some(result) => Some(result.clone()),
                None => {
                    let search_results = self
                        .client
                        .post("https://appapi.eshot.gov.tr/api/Assistant/getLineOrStationByName")
                        .headers(self.headers.clone())
                        .json(&line.code.to_string())
                        .send()
                        .await?
                        .json::<IzmSearchResponse>()
                        .await?;

                    search_results.data.clone().into_iter().for_each(|res| {
                        search_cache.insert(res);
                    });

                    search_results
                        .data
                        .into_iter()
                        .find(|res| *res.code == line.code)
                }
            };

            let Some(result) = search_result else {
                warn!(
                    "can't find {} in the cache or search results. Skipping and sleeping for 5 seconds",
                    line.code
                );

                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            };

            info!("getting line id: {}, code: {}", &result.id, result.code);
            let line_data = self
                .client
                .post("https://appapi.eshot.gov.tr/api/Assistant/getLine")
                .headers(self.headers.clone())
                .body(result.id.to_string())
                .send()
                .await?
                .json::<EshotLineResponse>()
                .await?;

            for route in line_data.data {
                let route_code = format!(
                    "{:?}_{:?}_D0",
                    &line.code,
                    Direction::try_from(route.direction).unwrap()
                );

                info!("inserting stops for {}", route_code);
                let insert_stops_result = QueryBuilder::new(
                    "INSERT INTO stops (stop_code, stop_name, x_coord, y_coord, city)",
                )
                .push_values(&route.stations, |mut b, station| {
                    b.push_bind(station.code.parse::<i32>().unwrap())
                        .push_bind(&station.name)
                        .push_bind(&station.lng)
                        .push_bind(&station.lat)
                        .push_bind("izmir");
                })
                .push(
                    "ON CONFLICT (stop_code, city) DO UPDATE SET
                            stop_name=EXCLUDED.stop_name,
                            x_coord=EXCLUDED.x_coord,
                            y_coord=EXCLUDED.y_coord",
                )
                .build()
                .execute(db)
                .await?;

                info!(
                    "inserted {} stops for {}",
                    insert_stops_result.rows_affected(),
                    route_code
                );

                info!("inserting line_stops for {}", route_code);
                let insert_line_stops_result = QueryBuilder::new(
                    "INSERT INTO line_stops (line_code, stop_code, route_code, city)",
                )
                .push_values(route.stations, |mut b, station| {
                    b.push_bind(&line.code)
                        .push_bind(station.id)
                        .push_bind(&route_code)
                        .push_bind("izmir");
                })
                .push("ON CONFLICT DO NOTHING")
                .build()
                .execute(db)
                .await?;

                info!(
                    "inserted {} line stops for {}",
                    insert_line_stops_result.rows_affected(),
                    &route_code
                );

                let mut latlngs: Vec<LatLng> = Vec::new();

                for line in route.tracks {
                    let pairs = line.split_whitespace();
                    for pair in pairs {
                        let mut coords = pair.split(",");
                        if let (Some(y), Some(x)) = (coords.next(), coords.next()) {
                            let x_parsed = x.parse::<f64>().unwrap();
                            let y_parsed = y.parse::<f64>().unwrap();

                            latlngs.push(LatLng {
                                lng: x_parsed,
                                lat: y_parsed,
                            });
                        }
                    }
                }

                let insert_route_paths = sqlx::query!(
                    r#"
                        INSERT INTO
                            route_paths (route_code, route_path, city)
                        VALUES
                            ($1, $2, $3)
                        ON CONFLICT (route_code, city) DO UPDATE SET
                            route_path=EXCLUDED.route_path
                    "#,
                    &route_code,
                    Json(latlngs) as _,
                    "izmir"
                )
                .execute(db)
                .await?;

                info!(
                    "inserted {} route paths for {}",
                    insert_route_paths.rows_affected(),
                    &route_code
                );

                info!("inserting timetable");
                let sunday = 0b1000000;
                let weekday = 0b0011111;
                let saturday = 0b0100000;

                let mut timetable = DatabaseTimetable {
                    city: "izmir".to_string(),
                    route_code,
                    ..Default::default()
                };

                for table in route.times {
                    let Ok(to_insert) = NaiveTime::from_str(&table.time) else {
                        continue;
                    };

                    if (table.day & weekday) != 0 {
                        timetable.monday.push(to_insert);
                        timetable.tuesday.push(to_insert);
                        timetable.wednesday.push(to_insert);
                        timetable.thursday.push(to_insert);
                        timetable.friday.push(to_insert);
                    }
                    if (table.day & sunday) != 0 {
                        timetable.sunday.push(to_insert);
                    }
                    if (table.day & saturday) != 0 {
                        timetable.saturday.push(to_insert);
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
                    timetable.route_code,
                    timetable.city,
                    &timetable.sunday,
                    &timetable.monday,
                    &timetable.tuesday,
                    &timetable.wednesday,
                    &timetable.thursday,
                    &timetable.friday,
                    &timetable.saturday
                )
                    .execute(db)
                    .await?;

                info!(
                    "inserted {} timetable row",
                    inserted_timetable.rows_affected()
                );
            }

            info!("sleeping for 10 seconds");
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }

        Ok(())
    }
}
