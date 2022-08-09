use actix_todo_sqlx::model::ConnectionPool;
use actix_web::middleware::Logger;
use actix_web::web::Data;
use actix_web::{App, HttpServer};
use std::error::Error;

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let db_pool = Data::new(
        ConnectionPool::connect("postgresql://actix-sqlx:dummy@localhost:5432/actix-sqlx").await?,
    );
    HttpServer::new(move || {
        App::new()
            .wrap(Logger::default())
            .app_data(db_pool.clone())
            .configure(actix_todo_sqlx::server::configure_app)
    })
    .bind("127.0.0.1:9000")?
    .run()
    .await?;

    Ok(())
}
