use crate::models::{Channel, File, NewChannel, Post};
use anyhow::Result;
use rust_tdlib::client::tdlib_client::TdJson;
use rust_tdlib::client::{Client, ClientState, ConsoleAuthStateHandler, Worker};
use rust_tdlib::types::{
    FileType, FormattedText, GetChatHistory, MessageContent, SearchPublicChat, TdlibParameters,
    TextEntity, TextEntityType, Update, DownloadFile,
};
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use time::Timespec;
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, RwLock, RwLockReadGuard};
use tokio::task::JoinHandle;
use warp::any;

mod parsers;
mod types;

const SEND_UPDATE_TIMEOUT: Duration = Duration::new(15, 0);

#[derive(Debug)]
pub enum NewUpdate {
    Post(Post),
    Channel(NewChannel),
    File(File)
}

#[derive(Clone)]
pub struct TelegramService {
    api_hash: String,
    app_id: i32,
    phone_number: String,
    inner: Arc<RwLock<Option<Inner>>>,
}

struct Inner {
    pub join_handle: JoinHandle<()>,
    pub client: Client<TdJson>,
    pub worker: Worker<ConsoleAuthStateHandler, TdJson>,
}

impl Inner {
    pub(self) fn new(
        join_handle: JoinHandle<()>,
        client: Client<TdJson>,
        worker: Worker<ConsoleAuthStateHandler, TdJson>,
    ) -> Self {
        Self {
            join_handle,
            client,
            worker,
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

    pub async fn start(&self) -> Result<Receiver<NewUpdate>> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<Box<Update>>(100);

        let client = Client::builder()
            .with_tdlib_parameters(
                TdlibParameters::builder()
                    .database_directory("tdlib")
                    .use_test_dc(true)
                    .api_id(self.app_id)
                    .api_hash(self.app_hash)
                    .system_language_code("en")
                    .device_model("Unknown")
                    .system_version("Unknown")
                    .application_version("0.0.1")
                    .enable_storage_optimizer(true)
                    .build(),
            )
            .with_updates_sender(sender)
            .build()?;

        let receiver = init_updates_reader(receiver);

        let mut worker = Worker::builder().build()?;
        let worker_waiter = worker.start();

        let client = worker.bind_client(client)?;

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = worker_waiter => log::info!("worker stopped"),
                _ = reader_waiter => log::info!("updates reader stopped")
            };
        });

        let mut inner = self.inner.write().await;
        if inner.is_some() {
            anyhow::bail!("service already started")
        }

        inner.insert(Inner::new(handle, client, worker));
        Ok(receiver)
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut guard = self.inner.write().await;
        if let Some(inner) = guard.take() {
            inner.worker.stop();
            if let Err(err) = &inner.join_handle.await? {
                log::error!("{}", err);
            }
        }
        Ok(())
    }

    pub async fn get_channel_history(&self, chat_id: i64) -> anyhow::Result<Vec<Post>> {
        let guard = self.inner.read().await;
        match guard.as_ref() {
            None => {
                anyhow::bail!("client not started yet")
            }
            Some(inner) => {
                let history = inner
                    .client
                    .get_chat_history(
                        GetChatHistory::builder()
                            .chat_id(chat_id)
                            .limit(100)
                            .offset(-50)
                            .from_message_id(0)
                            .build(),
                    )
                    .await?;
                log::info!("found history: {}", history.messages().len());
                Ok(history
                    .messages()
                    .into_iter()
                    .filter_map(|m| match m {
                        None => None,
                        Some(m) => Some(Post {
                            title: None,
                            link: "".to_string(),
                            telegram_id: m.id(),
                            pub_date: Timespec::new(m.date(), 0),
                            content: "".to_string(),
                            chat_id,
                        }),
                    })
                    .collect())
            }
        }
    }

    pub async fn download_file(&self, file_id: i32) -> anyhow::Result<()> {
        let mb_inner = self.inner.read().await;
        match mb_inner.as_ref() {
            None => {
                anyhow::bail!("service not started yet")
            }
            Some(inner) => {
                inner.client.download_file(DownloadFile::builder().file_id(file_id).build()).await?
            }
        }
        Ok(())
    }

    pub async fn search_channel(&self, channel_name: &str) -> anyhow::Result<Option<Channel>> {
        let mb_inner = self.inner.read().await;
        match mb_inner.as_ref() {
            None => {
                anyhow::bail!("service not started yet")
            }
            Some(inner) => {
                let channels = inner
                    .client
                    .search_public_chat(SearchPublicChat::builder().username(channel_name).build())
                    .await?;
                log::info!("{:?}", channels);
                Ok(channels.into_iter().find(|ch| ch.username.eq(channel_name)))
            }
        }
    }
}


fn init_updates_reader(mut receiver: Receiver<Box<Update>>) -> Receiver<NewUpdate> {
    let (sx, rx) = mpsc::channel(20);

    tokio::spawn(async move {
        while let Some(update) = receiver.recv().await {
            let new_update = match update {
                Update::ChatPhoto(chat_photo) => {None}
                Update::ChatTitle(chat_title) => {None}
                Update::File(file) => {None}
                Update::MessageContent(content) => {None}
                Update::NewChat(new_chat) => {None}
                Update::NewMessage(new_message) => {
                    let parsed = parsers::parse_message_content(new_message.message().content());
                    if let Some((content, file)) = parsed {
                        if let Some(file) = file {
                            sx.send_timeout(NewUpdate::File(file), SEND_UPDATE_TIMEOUT)
                        }
                        Some(NewUpdate::Post(Post {
                            title: None,
                            link: "".to_string(),
                            telegram_id: 0,
                            pub_date: "".to_string(),
                            content: content.unwrap_or_default(),
                            chat_id: 0
                        }))
                    }
                }
                Update::Poll(poll) => {None}
                _ => {}
            };
            if let Some(new_update) = new_update {
                sx.send_timeout(new_update, SEND_UPDATE_TIMEOUT)
            }

        }
    });

    rx
}