use crate::db::{Channel, DbService, NewChannel, Post};
use crate::models::File;
use crate::telegram::{NewUpdate, TelegramService};
use std::future::Future;
use std::sync::Arc;
use anyhow::Context;

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

    pub async fn start(&self) -> anyhow::Result<()> {
        log::info!("starting telegram service");
        let mut updates = self.inner.tg.start().await?;
        log::info!("telegram service started");

        let inner = self.inner.clone();
        tokio::spawn(async move {
            while let Some(update) = updates.recv().await {
                log::info!("new update: {:?}", update);
                match update {
                    NewUpdate::Post(post) => {
                        if let Err(err) = inner.db.save_channel_posts(vec![post]).await {
                            log::error!("cannot save channel posts: {}", err)
                        };
                    }
                    NewUpdate::Channel(channel) => {
                        if let Err(err) = inner.db.save_channel(channel).await {
                            log::error!("cannot save channel: {}", err)
                        };
                    }
                    NewUpdate::File(new_file) => {
                        match inner.db.get_file(&new_file).await {
                            Err(err) => {
                                log::error!("cannot get file: {}", err)
                            }
                            Ok(db_file) => {
                                match &db_file {
                                    None => {
                                        if let Err(err) =
                                            inner.tg.download_file(new_file.remote_file).await
                                        {
                                            log::error!("cannot download file: {}", err);
                                        }
                                    }
                                    Some(db_file) => {
                                        if db_file.local_path.is_none()
                                            && new_file.local_path.is_some()
                                        {
                                            // TODO: notify that file downloaded and post can be shown
                                        }
                                    }
                                }
                                if db_file.is_none() || db_file.unwrap().ne(&new_file) {
                                    if let Err(err) = inner.db.save_file(&new_file).await {
                                        log::error!("cannot save file: {}", err);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
        Ok(())
    }

    pub async fn synchronize_channels(&self) -> anyhow::Result<()> {
        let channels = self.inner.tg.get_all_channels().await?;
        for channel in channels.into_iter() {
            self.inner.db.save_channel(channel).await?;
        }
        Ok(())
    }

    pub async fn synchronize_files(&self) -> anyhow::Result<()> {
        let files = self.inner.db.get_not_loaded_files().await?;
        for f in files.iter() {
            if let Err(err) = self.inner.tg.download_file(f.remote_file).await {
                log::error!("file {} downloading failed: {}", f.remote_file, err)
            };
        }
        Ok(())

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
                .value(p.telegram_id().to_string())
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
            .title(channel.title.clone())
            // .description(channel.description().clone().unwrap_or_default()) TODO: description
            // .link(channel.link().clone().to_string()) TODO: link
            .items(items)
            .build()
            .map_err(|e| anyhow::anyhow!("error during building feed: {}", e))?;
        Ok(Some(feed))
    }

    async fn get_new_channel(&self, channel_name: &str) -> anyhow::Result<Option<(Channel)>> {
        match self.inner.tg.search_channel(channel_name).await? {
            None => Ok(None),
            Some(ch) => {
                let messages = self.inner.tg.get_channel_history(ch.telegram_id).await?;

                self.inner.db.save_channel(ch).await?;

                let saved_channel = match self.inner.db.get_channel(channel_name).await? {
                    None => {
                        log::info!("nothing found");
                        return Ok(None);
                    }
                    Some(ch) => ch,
                };
                log::info!("{:?}", saved_channel);

                self.inner.db.save_channel_posts(messages).await?;
                Ok(Some(saved_channel))
            }
        }
    }
}

fn rss_err<E: std::fmt::Debug>(err: E) -> anyhow::Error {
    anyhow::anyhow!("error building rss feed: {:?}", err)
}
