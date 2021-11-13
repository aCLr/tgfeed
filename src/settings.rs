use config::{Config, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DbSettings {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct TelegramSettings {
    pub api_hash: String,
    pub api_id: i32,
    pub phone: String,
}

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub telegram: TelegramSettings,
    pub db: DbSettings,
}

impl Settings {
    pub fn new() -> anyhow::Result<Self> {
        let mut s = Config::default();
        s.merge(File::with_name("config/default").required(false))?;
        s.merge(File::with_name("config/local").required(false))?;
        Ok(s.try_into()?)
    }
}
