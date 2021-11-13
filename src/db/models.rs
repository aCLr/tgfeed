#[derive(Debug, sqlx::FromRow)]
pub struct Post {
    pub(crate) title: Option<String>,
    pub(crate) link: String,
    pub(crate) guid: String,
    pub(crate) pub_date: String,
    pub(crate) content: String,
    pub(crate) channel_id: i64,
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
    pub(crate) title: String,
    pub(crate) username: String,
    pub(crate) link: String,
    pub(crate) description: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Channel {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) username: String,
    pub(crate) link: String,
    pub(crate) description: Option<String>,
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
