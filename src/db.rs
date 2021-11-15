pub use crate::models::{Channel, NewChannel, Post};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use crate::models::File;

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
            ON CONFLICT(remote_id) DO UPDATE SET remote_file = excluded.remote_file, local_path=excluded.local_path"#,
            file.local_path, file.remote_file, file.remote_id
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_file(&self, file: &File) -> anyhow::Result<Option<File>> {
        Ok(sqlx::query_as!(
            File,
            "SELECT * FROM files WHERE remote_id = $1",
            file.remote_id
        )
            .fetch_optional(&self.pool)
            .await?)
    }

    pub async fn save_channel(&self, channel: NewChannel) -> anyhow::Result<()> {
        sqlx::query_as!(
            Channel,
            r#"INSERT INTO channels (title, link, description, username)
            VALUES ($1, $2, $3, $4)"#,
            channel.title,
            channel.link,
            channel.description,
            channel.username
        )
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn save_channel_posts(&self, posts: Vec<Post>) -> anyhow::Result<()> {
        for p in posts.iter() {
            sqlx::query!(
                r#"INSERT INTO posts (title, link, guid, pub_date, content, channel_id)
                VALUES ($1, $2, $3, $4, $5, $6)"#,
                p.title,
                p.link,
                p.guid,
                p.pub_date,
                p.content,
                p.channel_id
            )
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn get_channel(&self, channel_name: &str) -> anyhow::Result<Option<Channel>> {
        Ok(sqlx::query_as!(
            Channel,
            "SELECT id, title, link, username, description from channels where username = $1",
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
            r#"SELECT title, link, guid, cast(datetime(pub_date) as text) as "pub_date!: String", content, channel_id
            FROM posts
            WHERE channel_id = $1
            LIMIT 25"#,
            ch.id
        )
            // .bind(ch.id)
            .fetch_all(&self.pool)
            .await {
            Ok(p) => p,
            Err(e) => {
                log::error!("{:?}", e);
                anyhow::bail!(e)
            }
        };
        Ok(Some((ch, posts)))
    }
}
