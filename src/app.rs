use crate::db::{Channel, DbService, NewChannel, Post};
use crate::telegram::{NewUpdate, TelegramService};
use std::future::Future;
use std::sync::Arc;
use tg_collector::parsers::{DefaultTelegramParser, TelegramDataParser};
use tg_collector::types::FileType;
use crate::models::File;

struct Inner {
    tg: TelegramService,
    db: DbService,
}

#[derive(Clone)]
pub struct App {
    inner: Arc<Inner>,
}

impl App {
    pub fn new(tg: TelegramService, db: DbService) -> Self {
        Self {
            inner: Arc::new(Inner { tg, db }),
        }
    }

    pub async fn start(&self) {
        let mut updates = self.inner
            .tg
            .start()
            .await
            .expect("can't start telegram service");
        let inner = self.inner.clone();
        tokio::spawn(async move {
            while let Some(update) = updates.recv().await {
                match update {
                    NewUpdate::Post(post) => {
                        inner.db.save_channel_posts(vec![post]).await?;
                    }
                    NewUpdate::Channel(channel) => {
                        inner.db.save_channel(channel).await?;
                    }
                    NewUpdate::File(new_file) => {
                        let db_file = inner.db.get_file(&new_file).await?;

                        match db_file {
                            None => inner.tg.download_file(new_file.remote_file).await?,
                            Some(db_file) => {
                                if db_file.local_path.is_none() && new_file.local_path.is_some() {
                                    // TODO: notify that file downloaded and post can be shown
                                }
                            }
                        }
                        if db_file.is_none() || db_file.ne(&new_file) {
                            inner.db.save_file(&new_file).await?;
                        }
                    }
                }
            }
        });
    }

    pub async fn get_posts_or_search(
        &self,
        channel_name: &str,
    ) -> anyhow::Result<Option<rss::Channel>> {
        match self.get_channel_posts(channel_name).await? {
            None => {
                log::info!("posts not found, searching for new channel");
                match self.get_new_channel(channel_name).await? {
                    None => {
                        log::info!("channel not found");
                        Ok(None)
                    }
                    Some(_) => {
                        log::info!("found new channel, retrieving posts");
                        Ok(self.get_channel_posts(channel_name).await?)
                    }
                }
            }
            Some(p) => Ok(Some(p)),
        }
    }

    pub async fn get_channel_posts(
        &self,
        channel_name: &str,
    ) -> anyhow::Result<Option<rss::Channel>> {
        let (channel, posts) = match self.inner.db.get_channel_posts(channel_name).await? {
            None => return Ok(None),
            Some(ch) => ch,
        };
        let mut items = Vec::new();
        for p in posts.into_iter() {
            let guid = rss::GuidBuilder::default()
                .value(p.guid().to_string())
                .build()
                .map_err(rss_err)?;
            let item = rss::ItemBuilder::default()
                .title(p.title().clone().unwrap_or_default())
                .link(p.link().clone().to_string())
                .guid(Some(guid))
                .pub_date(p.pub_date().clone().to_string())
                .content(p.content().clone().to_string())
                .build()
                .map_err(rss_err)?;
            items.push(item);
        }
        let feed = rss::ChannelBuilder::default()
            .title(channel.title().clone().to_string())
            .description(channel.description().clone().unwrap_or_default())
            .link(channel.link().clone().to_string())
            .items(items)
            .build()
            .map_err(|e| anyhow::anyhow!("error during building feed: {}", e))?;
        Ok(Some(feed))
    }

    async fn get_new_channel(&self, channel_name: &str) -> anyhow::Result<Option<(Channel)>> {
        match self.inner.tg.search_channel(channel_name).await? {
            None => Ok(None),
            Some(ch) => {
                let messages = self.inner.tg.get_channel_history(&ch).await?;
                log::info!("found messages: {}", messages.len());
                self.inner
                    .db
                    .save_channel(NewChannel {
                        title: ch.title,
                        username: channel_name.to_string(),
                        link: "".to_string(),
                        description: {
                            let trimmed = ch.description.trim();
                            match trimmed.is_empty() {
                                true => None,
                                false => Some(trimmed.to_string()),
                            }
                        },
                    })
                    .await?;

                let saved_channel = match self.inner.db.get_channel(channel_name).await? {
                    None => {
                        log::info!("nothing found");
                        return Ok(None);
                    }
                    Some(ch) => ch,
                };
                log::info!("{:?}", saved_channel);

                let parser = DefaultTelegramParser::new();
                let mut posts = Vec::with_capacity(messages.len());
                for msg in messages.into_iter() {
                    let (content, files) = parser.parse_message_content(msg.content()).await?;
                    if let Some(content) = content {
                        posts.push(Post {
                            title: None,
                            link: "".to_string(),
                            guid: msg.id().to_string(),
                            pub_date: msg.date().to_string(),
                            channel_id: saved_channel.id,
                            content,
                        })
                    }

                    if let Some(files) = files {
                        for f in files.iter() {
                            match &f.file_type {
                                FileType::Document => {}
                                FileType::Audio(_) => {}
                                FileType::Video(_) => {}
                                FileType::Animation(_) => {}
                                FileType::Image(img) => {
                                    if let Err(err) = self
                                        .inner
                                        .tg
                                        .download_file(f.path.remote_file.parse().unwrap())
                                        .await
                                    {
                                        log::error!("downloading failed; message_id={}, channel_name={}, err={}", msg.id(), channel_name, err)
                                    }
                                }
                            }
                        }
                    }
                }
                self.inner.db.save_channel_posts(posts).await?;
                Ok(Some(saved_channel))
            }
        }
    }
}

fn rss_err<E: std::fmt::Debug>(err: E) -> anyhow::Error {
    anyhow::anyhow!("error building rss feed: {:?}", err)
}
