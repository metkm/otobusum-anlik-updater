#![allow(dead_code)]

use sqlx::PgPool;
use updater::Updater;
use clap::Parser;

mod models;
mod updater;
mod updaters;
mod constants;

#[derive(Parser, Debug)]
#[command(about)]
struct Args {
    /// istanbul, izmir
    #[arg(short, long)]
    city: Vec<String>,

    #[arg(long)]
    update_lines: bool,

    /// this might take a while for istanbul
    #[arg(long)]
    update_routes: bool,

    /// this might take a while for istanbul & izmir
    #[arg(long)]
    update_line_stops: bool,

    #[arg(long)]
    update_route_paths: bool,

    /// this might take a while for istanbul
    #[arg(long)]
    update_timetable: bool,

    #[arg(long, default_value_t = 0)]
    offset: usize,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    dotenv::dotenv().expect(".env file is required");
    tracing_subscriber::fmt().init();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
    let pool = PgPool::connect(&database_url).await?;

    if args.city.contains(&"istanbul".to_string()) {
        let mut ist_updater = updaters::ist::IstUpdater::new();
        ist_updater.get_credentials().await?;

        if args.update_lines {
            ist_updater.insert_lines(&pool).await?;
        }

        if args.update_routes {
            ist_updater.insert_routes(&pool, args.offset).await?;
        }

        if args.update_line_stops {
            ist_updater.insert_line_stops(&pool, args.offset).await?;
        }

        if args.update_route_paths {
            ist_updater.insert_route_paths(&pool).await?;
        }

        if args.update_timetable {
            ist_updater.insert_timetable(&pool, args.offset).await?;
        }
    }

    if args.city.contains(&"izmir".to_string()) {
        let mut izm_updater = updaters::izm::IzmUpdater::new();
        izm_updater.get_credentials().await?;
        
        if args.update_lines {
            izm_updater.insert_lines(&pool).await?;
        }

        if args.update_line_stops {
            izm_updater.insert_line_stops(&pool, args.offset).await?;
        }
    }

    Ok(())
}
