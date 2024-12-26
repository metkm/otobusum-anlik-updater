use updater::Updater;
use sqlx::PgPool;

mod models;
mod updater;
mod updaters;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().expect(".env file is required");
    
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
    let pool = PgPool::connect(&database_url).await?;

    let ist_updater = updaters::ist::IstUpdater::new();

    // ist_updater.get_credentials().await?;
    ist_updater.insert_lines(&pool).await?;

    Ok(())
}
