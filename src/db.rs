use crate::models::{File, TelegramChatId, TelegramPostId};
pub use crate::models::{Channel, NewChannel, Post};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use std::collections::HashMap;

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

    pub async fn get_files_for_posts(
        &self,
        post_ids: Vec<i32>,
    ) -> anyhow::Result<HashMap<i32, Vec<i32>>> {
        let rows = sqlx::query(
            r#"SELECT post_id, local_path, remote_file as "remote_file: i32", remote_id
                FROM files
                INNER JOIN post_files ON post_files.file_id=files.id
                WHERE post_files.post_id IN ($1)"#,
        )
        .bind(
            post_ids
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<String>>()
                .join(","),
        )
        .fetch_all(&self.pool)
        .await?;
        let mut result = HashMap::with_capacity(rows.len());
        for row in rows.into_iter() {
            let mut post_files: &mut Vec<i32> = result.entry(row.get("post_id")).or_default();
            post_files.push(row.get("remote_file"))
        }
        Ok(result)
    }

    pub async fn get_not_loaded_files(&self) -> anyhow::Result<Vec<File>> {
        Ok(sqlx::query_as!(
            File,
            r#"SELECT local_path, remote_file as "remote_file: i32", remote_id FROM files WHERE local_path is null"#,
        )
            .fetch_all(&self.pool)
            .await?)
    }

    pub async fn save_post_files(&self, post_id: i64, file_ids: Vec<i32>) -> anyhow::Result<()> {
        for file_id in file_ids.iter() {
            sqlx::query!(
                "insert into post_files (post_id, file_id) values ($1, $2)",
                post_id, file_id
            ).execute(&self.pool).await?;
        }
        Ok(())
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

    pub async fn save_channel_posts(&self, posts: &Vec<Post>) -> anyhow::Result<()> {
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

    pub async fn get_channel_post_ids(&self, chat_id: TelegramChatId, limit: i32) -> anyhow::Result<Vec<(i64, TelegramPostId)>> {
        let rows = sqlx::query!(
            r#"SELECT id, telegram_id
            FROM posts
            WHERE chat_id = $1
            LIMIT $2"#,
            chat_id,
            limit,
        ).fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(|v|(v.id, v.telegram_id)).collect())
    }

    pub async fn get_channel_posts(
        &self,
        channel_name: &str,
    ) -> anyhow::Result<Option<(Channel, Vec<Post>)>> {
        let ch = match self.get_channel(channel_name).await? {
            None => return Ok(None),
            Some(ch) => ch,
        };
        let rows = sqlx::query(
            r#"SELECT title, link, telegram_id, pub_date as "pub_date: i32", content, chat_id
            FROM posts
            WHERE chat_id = $1
            LIMIT 25"#,
            // ch.telegram_id
        )
        .bind(ch.telegram_id)
        .fetch_all(&self.pool)
        .await?;
        let mut files = self
            .get_files_for_posts(rows.iter().map(|r| r.get("id")).collect())
            .await?;
        let mut posts = Vec::with_capacity(rows.len());
        rows.into_iter().for_each(|r| {
            let post_files = files.remove(&r.get("id")).unwrap_or_default();
            let post = Post {
                title: r.get("title"),
                link: r.get("link"),
                telegram_id: r.get("telegram_id"),
                pub_date: r.get("pub_date"),
                content: r.get("content"),
                chat_id: r.get("chat_id"),
                files: post_files,
            };
            posts.push(post);
        });
        Ok(Some((ch, posts)))
    }
}
