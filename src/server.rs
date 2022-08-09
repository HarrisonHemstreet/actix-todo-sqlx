use crate::model::{ConnectionPool, Todo};

use actix_web::web::ServiceConfig;
use actix_web::{web, App, Responder};
use serde::de::IntoDeserializer;

pub fn configure_app(config: &mut ServiceConfig) {
    config.service(web::scope("/api/v1").configure(todos_service));
}

#[derive(serde::Deserialize)]
struct SearchQuery {
    search: String,
}

#[derive(serde::Deserialize)]
struct CreateTodo {
    todo: String,
}

fn todos_service(config: &mut ServiceConfig) {
    config.service(
        web::scope("/todos")
            .route("/", web::to(all_todos))
            .route(
                "/search",
                web::to(|data, search: web::Query<SearchQuery>| async move {
                    search_todos(data, search.into_inner()).await
                }),
            )
            .route(
                "/filter/{done}",
                web::to(|data, done: web::Path<String>| async move {
                    filter_todos(data, done.into_inner().as_str() == "done").await
                }),
            )
            .route("/create", web::post().to(create_todos)),
    );
}

async fn create_todos(
    data: web::Data<ConnectionPool>,
    name: web::Json<CreateTodo>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let mut conn = data.acquire().await?;
    let todos = Todo::create_todo(&mut conn, name.into_inner().todo, false).await?;

    Ok(web::Json(todos))
}
async fn all_todos(
    data: web::Data<ConnectionPool>,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let mut conn = data.acquire().await?;
    let todos = Todo::get_all_todos(&mut conn).await?;

    Ok(web::Json(todos))
}

async fn search_todos(
    data: web::Data<ConnectionPool>,
    search: SearchQuery,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let mut conn = data.acquire().await?;
    let todos = Todo::search_todos(&mut conn, &search.search).await?;

    // String -> Deserializer -> Deserialize/Serialize -> Serializer -> String
    // String    Deserializer   ---------------------->   Serializer -> String
    // Input     serde_json          Todo                 serde_json    Output
    Ok(web::Json(serde_transcode::Transcoder::new(
        todos.into_deserializer(),
    )))
}

async fn filter_todos(
    data: web::Data<ConnectionPool>,
    done: bool,
) -> Result<impl Responder, Box<dyn std::error::Error>> {
    let mut conn = data.acquire().await?;
    let todos = Todo::filter_todos(&mut conn, done).await?;
    Ok(web::Json(todos))
}
