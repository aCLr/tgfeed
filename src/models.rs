use rust_tdlib::types::{Chat, File as TgFile};

pub type TelegramPostId = i64;
pub type TelegramChatId = i64;
#[derive(Debug, sqlx::FromRow)]
pub struct Post {
    pub title: Option<String>,
    pub link: String,
    pub telegram_id: TelegramPostId,
    pub pub_date: i32,
    pub content: String,
    pub chat_id: TelegramChatId,
}

impl Post {
    pub fn title(&self) -> &Option<String> {
        &self.title
    }
    pub fn link(&self) -> &str {
        &self.link
    }
    pub fn telegram_id(&self) -> TelegramPostId {
        self.telegram_id
    }
    pub fn pub_date(&self) -> i32 {
        self.pub_date
    }
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn chat_id(&self) -> TelegramChatId {
        self.chat_id
    }
}

#[derive(Debug)]
pub struct NewChannel {
    pub title: String,
    pub telegram_id: TelegramChatId,
    pub username: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Channel {
    pub id: i64,
    pub title: String,
    pub username: String,
    pub telegram_id: TelegramChatId,
}

#[derive(Debug, PartialEq)]
pub struct File {
    pub local_path: Option<String>,
    // id to download
    pub remote_file: i32,
    // id to make requests
    pub remote_id: String,
    pub telegram_post_id:  TelegramPostId,
}


impl From<&TgFile> for File {
    fn from(file: &TgFile) -> Self {
        Self {
            local_path: Some(file.local().path().as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string()),
            remote_file: file.id(),
            remote_id: file.remote().unique_id().clone(),
        }
    }
}