#[derive(Debug, sqlx::FromRow)]
pub struct Post {
    pub title: Option<String>,
    pub link: String,
    pub telegram_id: i64,
    pub pub_date: String,
    pub content: String,
    pub chat_id: i64,
}

impl Post {
    pub fn title(&self) -> &Option<String> {
        &self.title
    }
    pub fn link(&self) -> &str {
        &self.link
    }
    pub fn guid(&self) -> &str {
        &self.guid
    }
    pub fn pub_date(&self) -> &str {
        &self.pub_date
    }
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn channel_id(&self) -> i64 {
        self.channel_id
    }
}

#[derive(Debug)]
pub struct NewChannel {
    pub title: String,
    pub username: String,
    pub link: String,
    pub description: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Channel {
    pub id: i64,
    pub title: String,
    pub username: String,
    pub link: String,
    pub description: Option<String>,
}

impl Channel {
    pub fn id(&self) -> i64 {
        self.id
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn username(&self) -> &str {
        &self.username
    }
    pub fn link(&self) -> &str {
        &self.link
    }
    pub fn description(&self) -> &Option<String> {
        &self.description
    }
}

#[derive(Debug)]
pub struct File {
    pub local_path: Option<String>,
    // id to download
    pub remote_file: i32,
    // id to make requests
    pub remote_id: String,
}