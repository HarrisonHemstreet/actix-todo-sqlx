CREATE TABLE IF NOT EXISTS todo_todos (
    id  serial primary key,
    name varchar NOT NULL,
    done boolean NOT NULL
)