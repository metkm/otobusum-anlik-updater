use sqlx::PgPool;
use updater::Updater;

mod models;
mod updater;
mod updaters;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().expect(".env file is required");
    tracing_subscriber::fmt().init();

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
    let pool = PgPool::connect(&database_url).await?;

    // let mut izm_updater = updaters::izm::IzmUpdater::new();

    // izm_updater.insert_lines(&pool).await?;
    // izm_updater.get_credentials().await?;
    // izm_updater.insert_line_stops(&pool).await?;

    let ist_updater = updaters::ist::IstUpdater::new();

    // ist_updater.insert_lines(&pool).await?;
    // ist_updater.get_credentials().await?;
    // ist_updater.insert_routes(&pool).await?;
    // ist_updater.insert_line_stops(&pool).await?;
    ist_updater.insert_route_paths(&pool).await?;
    // ist_updater.insert_timetable(&pool).await?;

    Ok(())
}
