use anyhow::Result;
use sqlx::SqlitePool;

pub(crate) struct Db {
    pool: SqlitePool,
}

impl Db {
    pub(crate) async fn add_message(&self, message: &str) -> Result<()> {
        sqlx::query("INSERT INTO messages (message) VALUES (?)")
            .bind(message)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub(crate) async fn list_messages(&self, limit: i64) -> Result<Vec<String>> {
        let messages = sqlx::query_as::<_, (String,)>(&format!(
            "SELECT message FROM messages
            ORDER BY id DESC
            LIMIT {limit}
        "
        ))
        .fetch_all(&self.pool)
        .await?;

        Ok(messages.into_iter().map(|(message,)| message).collect())
    }
}

pub(crate) async fn init_db() -> Result<Db> {
    let pool = SqlitePool::connect("sqlite:db.sqlite?mode=rwc").await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message TEXT NOT NULL
        )
        "#,
    )
    .execute(&pool)
    .await?;

    Ok(Db { pool })
}
