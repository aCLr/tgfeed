use std::future::Future;
use crate::db::{DbService, NewChannel, Post, Channel};
use crate::telegram::{TelegramService};
use std::sync::Arc;
use tg_collector::parsers::{DefaultTelegramParser, TelegramDataParser};

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

    pub async fn get_posts_or_search(&self, channel_name: &str) -> anyhow::Result<Option<rss::Channel>> {
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
            Some(p) => {Ok(Some(p))}
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

    async fn get_new_channel(&self, channel_name: &str) -> anyhow::Result<Option<(Channel)>>{
        match self
            .inner
            .tg
            .search_channel(channel_name)
            .await? {
            None => {Ok(None)}
            Some(ch) => {
                let messages = self.inner.tg.get_channel_history(&ch).await?;
                log::info!("found messages: {}", messages.len());
                self.inner.db.save_channel(NewChannel{
                    title: ch.title,
                    username: channel_name.to_string(),
                    link: "".to_string(),
                    description: {
                        let trimmed = ch.description.trim();
                        match trimmed.is_empty() {
                            true => {None}
                            false => {Some(trimmed.to_string())}
                        }
                    },
                }).await?;

                let saved_channel = match self.inner.db.get_channel(channel_name).await? {
                    None => {
                        log::info!("nothing found");
                        return Ok(None);
                    },
                    Some(ch) => ch,
                };
                log::info!("{:?}", saved_channel);

                let parser = DefaultTelegramParser::new();
                let mut posts = Vec::with_capacity(messages.len());
                for msg in messages.into_iter() {
                    let content = parser.parse_message_content(msg.content()).await?.0.unwrap_or_default();
                    posts.push(Post {
                        title: None,
                        link: "".to_string(),
                        guid: msg.id().to_string(),
                        pub_date: msg.date().to_string(),
                        content,
                        channel_id: saved_channel.id,
                    })
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
