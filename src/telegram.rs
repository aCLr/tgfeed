use anyhow::Result;
use std::ops::Deref;
use std::sync::Arc;
use tg_collector::{
    Message,
    parsers::{DefaultTelegramParser, TelegramDataParser},
    tg_client::TgClient,
};
use tokio::sync::{mpsc, RwLock, RwLockReadGuard};
use tokio::task::JoinHandle;
use warp::any;

pub use tg_collector::types::Channel;

#[derive(Clone)]
pub struct TelegramService {
    api_hash: String,
    app_id: i32,
    phone_number: String,
    inner: Arc<RwLock<Option<Inner>>>,
}

struct Inner {
    pub join_handle: JoinHandle<anyhow::Result<()>>,
    pub client: TgClient,
}

impl Inner {
    pub(self) fn new(join_handle: JoinHandle<anyhow::Result<()>>, client: TgClient) -> Self {
        Self {
            join_handle,
            client,
        }
    }
}

impl TelegramService {
    pub fn new(api_hash: String, app_id: i32, phone_number: String) -> Self {
        Self {
            api_hash,
            app_id,
            phone_number,
            inner: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn start(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if inner.is_some() {
            anyhow::bail!("service already started")
        }

        let (sx, mut rx) = mpsc::channel(10);
        let mut client = TgClient::builder()
            .with_api_hash(self.api_hash.clone())
            .with_api_id(self.app_id)
            .with_phone_number(self.phone_number.clone())
            .with_encryption_key("".to_string())
            .build()?;

        log::debug!("starting updates listening");
        client.start_listen_updates(sx)?;
        log::debug!("starting client");
        let handle = client.start().await?;
        log::debug!("client started");

        let handle = tokio::spawn(async move {
            handle.await??;
            Ok(())
        });

        tokio::spawn(async move {
            let parser = DefaultTelegramParser::new();
            while let Some(msg) = rx.recv().await {
                let res = parser.parse_update(&msg).await;
                log::error!("{:?}", res);
            }
        });
        inner.insert(Inner::new(handle, client));
        Ok(())
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut guard = self.inner.write().await;
        if let Some(inner) = guard.take() {
            if let Err(err) = &inner.join_handle.await? {
                log::error!("{}", err);
            }
        }
        Ok(())
    }

    pub async fn get_channel_history(&self, channel: &Channel) -> anyhow::Result<Vec<Message>> {
        let guard = self.inner.read().await;
        match guard.as_ref() {
            None => {
                anyhow::bail!("client not started yet")
            }
            Some(inner) => {
                let history = inner
                    .client
                    .get_chat_history(channel.chat_id, -50, 100, 0)
                    .await?;
                log::info!("found history: {}", history.messages().len());
                Ok(history.messages().into_iter().filter_map(|m| {
                    match m {
                        None => {None}
                        Some(m) => {Some(m.clone())}
                    }
                }).collect())
            }
        }
    }

    pub async fn search_channel(&self, channel_name: &str) -> anyhow::Result<Option<Channel>> {
        let mb_inner = self.inner.read().await;
        match mb_inner.as_ref() {
            None => {
                anyhow::bail!("service not started yet")
            }
            Some(inner) => {
                let channels = inner.client.search_public_chat(channel_name).await?;
                log::info!("{:?}", channels);
                Ok(channels.into_iter().find(|ch| ch.username.eq(channel_name)))
            }
        }
    }
}
