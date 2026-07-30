use sqlx::PgPool;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn connect(url: &str) -> sqlx::Result<PgPool> {
    // ponytail: 5 соединений хватает надолго; поднимать, когда упрёмся в пул
    PgPoolOptions::new().max_connections(5).connect(url).await
}
