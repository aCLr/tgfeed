use crate::models::File;
pub use crate::models::{Channel, NewChannel, Post};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

pub struct DbService {
    pool: SqlitePool,
}

impl DbService {
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(3)
            .connect(db_path)
            .await?;
        Ok(Self { pool })
    }

    pub async fn save_file(&self, file: &File) -> anyhow::Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO files (local_path, remote_file, remote_id) VALUES ($1, $2, $3)
            ON CONFLICT(remote_file) DO UPDATE SET remote_id = excluded.remote_id, local_path=excluded.local_path"#,
            file.local_path, file.remote_file, file.remote_id
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_file(&self, file: &File) -> anyhow::Result<Option<File>> {
        Ok(sqlx::query_as!(
            File,
            r#"SELECT local_path, remote_file as "remote_file: i32", remote_id FROM files WHERE remote_id = $1"#,
            file.remote_id
        )
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn get_not_loaded_files(&self) -> anyhow::Result<Vec<File>> {
        Ok(sqlx::query_as!(
            File,
            r#"SELECT local_path, remote_file as "remote_file: i32", remote_id FROM files WHERE local_path is null"#,
        )
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn save_channel(&self, channel: NewChannel) -> anyhow::Result<()> {
        sqlx::query_as!(
            Channel,
            r#"INSERT INTO channels (title, username, telegram_id)
            VALUES ($1, $2, $3)
            ON CONFLICT(username) DO UPDATE SET title = excluded.title, telegram_id=excluded.telegram_id"#,
            channel.title,
            channel.username,
            channel.telegram_id,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn save_channel_posts(&self, posts: Vec<Post>) -> anyhow::Result<()> {
        for p in posts.iter() {
            sqlx::query!(
                r#"INSERT INTO posts (title, link, telegram_id, pub_date, content, chat_id)
                VALUES ($1, $2, $3, $4, $5, $6)"#,
                p.title,
                p.link,
                p.telegram_id,
                p.pub_date,
                p.content,
                p.chat_id
            )
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn get_channel(&self, channel_name: &str) -> anyhow::Result<Option<Channel>> {
        Ok(sqlx::query_as!(
            Channel,
            "SELECT id, title, username, telegram_id from channels where username = $1",
            channel_name
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn get_channel_posts(
        &self,
        channel_name: &str,
    ) -> anyhow::Result<Option<(Channel, Vec<Post>)>> {
        let ch = match self.get_channel(channel_name).await? {
            None => return Ok(None),
            Some(ch) => ch,
        };
        let posts = match sqlx::query_as!(
            Post,
            r#"SELECT title, link, telegram_id, pub_date as "pub_date: i32", content, chat_id
            FROM posts
            WHERE chat_id = $1
            LIMIT 25"#,
            ch.telegram_id
        )
        // .bind(ch.id)
        .fetch_all(&self.pool)
        .await
        {
            Ok(p) => p,
            Err(e) => {
                log::error!("{:?}", e);
                anyhow::bail!(e)
            }
        };
        Ok(Some((ch, posts)))
    }
}
