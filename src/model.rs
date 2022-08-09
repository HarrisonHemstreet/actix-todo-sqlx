use crate::model::serde::DbRow;
use ::serde::de::IntoDeserializer;
use ::serde::{Deserialize, Deserializer, Serialize};
use sqlx::{query, PgConnection, Pool, Postgres};

pub type ConnectionPool = Pool<Postgres>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Todo {
    id: i32,
    name: String,
    done: bool,
}

impl Todo {
    pub async fn create_todo(
        connection: &mut PgConnection,
        name: impl AsRef<str>,
        done: bool,
    ) -> Result<Self, sqlx::Error> {
        sqlx::query_as!(
            Todo,
            "insert into todo_todos (name, done) values ($1, $2) returning id, name, done",
            name.as_ref(),
            done
        )
        .fetch_one(connection)
        .await
    }

    pub async fn get_all_todos(connection: &mut PgConnection) -> Result<Vec<Self>, sqlx::Error> {
        // Approach 1: use query_as! to serialize into a row object.
        let query: Vec<_> = sqlx::query_as!(Todo, "select * from todo_todos")
            .fetch_all(connection)
            .await?;
        Ok(query)
    }

    pub async fn filter_todos(
        connection: &mut PgConnection,
        done: bool,
    ) -> Result<Vec<Self>, sqlx::Error> {
        // Structural typing (duck typing)
        // {name: string, done: bool} === {name: string, done: bool}

        // Nominal typing, named typing
        // class Foo { }
        // def takes_foo(foo: Foo) {}

        // Approach 2: Manually map the values out of the query
        let query: Vec<_> = query!("select * from todo_todos where done = $1", done)
            .map(|row| Todo {
                id: row.id,
                name: row.name,
                done: row.done,
            })
            .fetch_all(connection)
            .await?;

        Ok(query)
    }

    pub async fn search_todos(
        connection: &mut PgConnection,
        search: &str,
    ) -> Result<Vec<DbRow>, sqlx::Error> {
        // Option 3: Domain specific serde implementation for transcoding
        let query: Vec<_> = sqlx::query(r#"select * from todo_todos where name like $1"#)
            .bind(format!("%{}%", search))
            .map(DbRow)
            .fetch_all(connection)
            .await?;

        Ok(query)
    }
}

mod serde;

#[cfg(test)]
mod tests {
    use crate::model::{ConnectionPool, Todo};
    use serde::de::IntoDeserializer;
    use sqlx::{Connection, PgConnection};
    use std::io::BufWriter;

    macro_rules! db_test {
        ($test:path) => {
            let pool = ConnectionPool::connect(TEST_DB_URL).await.unwrap();
            let mut conn = pool.acquire().await.unwrap();
            let _: Result<(), sqlx::Error> = conn
                .transaction(|trans| {
                    Box::pin(async move {
                        let value = $test(trans).await;
                        assert!(value.is_ok());
                        Err(sqlx::Error::RowNotFound)
                    })
                })
                .await;
        };
    }

    const TEST_DB_URL: &str = "postgresql://actix-sqlx:dummy@localhost:5432/actix-sqlx";

    #[tokio::test]
    async fn get_all_todos() {
        db_test![_get_all_todos];
    }

    #[tokio::test]
    async fn it_filters_todos() {
        db_test![filter_todos];
    }

    #[tokio::test]
    async fn it_searches_todos() {
        db_test!(search_todos);
    }

    async fn search_todos(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
        let not_done = Todo::create_todo(conn, "not done", false).await?;
        let done = Todo::create_todo(conn, "done", true).await?;

        let mut count = 0;

        let mut output = vec![];
        let todos = Todo::search_todos(conn, "t do").await?;
        let mut json = serde_json::Serializer::pretty(&mut output);
        let todos_deserializer = todos.into_deserializer();
        serde_transcode::transcode(todos_deserializer, &mut json).unwrap();

        // reparse the json
        let json_parsed: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(json_parsed.as_array().unwrap().len(), 1);
        // defaults to ordinal serialization so this looks like
        // [  [ id, name, done ],  ]
        assert_eq!(json_parsed[0][0], not_done.id);
        Ok(())
    }

    async fn filter_todos(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
        let not_done = Todo::create_todo(conn, "not done", false).await?;
        let done = Todo::create_todo(conn, "done", true).await?;

        let todos = Todo::filter_todos(conn, true).await?;

        assert_eq!(todos[0].id, done.id);
        Ok(())
    }

    async fn _get_all_todos(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
        for _ in 0..10_000 {
            let todo = Todo::create_todo(conn, "Some todo", false).await?;
        }
        let todos = Todo::get_all_todos(conn).await?;
        assert_eq!(todos.len(), 10_000, "{:?}", &todos);
        Ok(())
    }
}
