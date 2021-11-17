use crate::models::{Channel, File, NewChannel, Post};
use anyhow::Result;
use rust_tdlib::client::tdlib_client::TdJson;
use rust_tdlib::client::{
    AuthStateHandler, Client, ClientState, ConsoleAuthStateHandler, SignalAuthStateHandler, Worker,
};
use rust_tdlib::tdjson::set_log_verbosity_level;
use rust_tdlib::types::{AuthorizationStateWaitCode, AuthorizationStateWaitEncryptionKey, AuthorizationStateWaitOtherDeviceConfirmation, AuthorizationStateWaitPassword, AuthorizationStateWaitPhoneNumber, AuthorizationStateWaitRegistration, ChatType, DownloadFile, FileType, FormattedText, GetChatHistory, GetChats, MessageContent, SearchPublicChat, TdlibParameters, TextEntity, TextEntityType, Update, GetChat, Chat, GetSupergroup};
use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use time::{OffsetDateTime, PrimitiveDateTime};
use tokio::sync::mpsc::Receiver;
use tokio::sync::{mpsc, RwLock, RwLockReadGuard};
use tokio::task::JoinHandle;
use warp::any;

mod parsers;

const SEND_UPDATE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug)]
pub enum NewUpdate {
    Post(Post),
    Channel(NewChannel),
    File(File),
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
    pub worker: Worker<AuthHandler, TdJson>,
}

impl Inner {
    pub(self) fn new(
        join_handle: JoinHandle<()>,
        client: Client<TdJson>,
        worker: Worker<AuthHandler, TdJson>,
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
        set_log_verbosity_level(1);
        let (sender, receiver) = tokio::sync::mpsc::channel::<Box<Update>>(100);

        let client = Client::builder()
            .with_tdlib_parameters(
                TdlibParameters::builder()
                    .database_directory("tdlib")
                    .use_test_dc(false)
                    .api_id(self.app_id)
                    .api_hash(self.api_hash.clone())
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

        let mut worker = Worker::builder()
            .with_auth_state_handler(AuthHandler::new("", self.phone_number.as_str()))
            .build()?;
        let worker_waiter = worker.start();

        let client = worker.bind_client(client).await?;

        let handle = tokio::spawn(async move {
            tokio::select! {
                _ = worker_waiter => log::info!("worker stopped"),
            };
        });

        let mut inner = self.inner.write().await;
        if inner.is_some() {
            anyhow::bail!("service already started")
        }

        inner.insert(Inner::new(handle, client, worker));
        Ok(receiver)
    }

    pub async fn stop(&self) {
        let mut guard = self.inner.write().await;
        if let Some(inner) = guard.take() {
            inner.worker.stop();
            if let Err(err) = &inner.join_handle.await {
                log::error!("{}", err);
            }
        }
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

                let mut result = Vec::with_capacity(history.messages().len());
                for msg in history.messages().into_iter() {
                    if let Some(msg) = msg {
                        let (content, file) = match parsers::parse_message_content(msg.content()) {
                            None => (None, None),
                            Some((content, file)) => (content, file),
                        };

                        if let Some(file) = file {
                            if let Err(err) = self.download_file(file.remote_file).await {
                                log::error!("cannot download file: {}", err);
                            }
                        }
                        result.push(Post {
                            title: None,
                            link: "".to_string(),
                            telegram_id: msg.id(),
                            pub_date: msg.date(),
                            content: content.unwrap_or_default(),
                            chat_id,
                        })
                    }
                }
                Ok(result)
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
                log::info!("downloading file {}", file_id);
                inner
                    .client
                    .download_file(DownloadFile::builder().file_id(file_id).priority(1).build())
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn get_all_channels(&self) -> anyhow::Result<Vec<NewChannel>> {
        let mb_inner = self.inner.read().await;
        match mb_inner.as_ref() {
            None => {
                anyhow::bail!("service not started yet")
            }
            Some(inner) => {
                let chats = inner
                    .client
                    .get_chats(
                        GetChats::builder()
                            .limit(9999)
                            .offset_order(9223372036854775807)
                            .build(),
                    )
                    .await?;
                let mut result = Vec::with_capacity(chats.chat_ids().len());
                for chat_id in chats.chat_ids().into_iter() {
                    let chat = inner.client.get_chat(GetChat::builder().chat_id(*chat_id).build()).await?;

                    if let ChatType::Supergroup(type_sg) = chat.type_() {
                        if type_sg.is_channel() {
                            let sg = inner.client.get_supergroup(GetSupergroup::builder().supergroup_id(type_sg.supergroup_id()).build()).await?;

                            result.push(new_channel(chat, sg.username()))

                        }
                    }

                }
                Ok(result)
            }
        }
    }

    pub async fn search_channel(&self, channel_name: &str) -> anyhow::Result<Option<NewChannel>> {
        let mb_inner = self.inner.read().await;
        match mb_inner.as_ref() {
            None => {
                anyhow::bail!("service not started yet")
            }
            Some(inner) => {
                let chat = inner
                    .client
                    .search_public_chat(SearchPublicChat::builder().username(channel_name).build())
                    .await?;
                if !is_channel(&chat) {
                    return Ok(None)
                }

                Ok(Some(new_channel(chat, channel_name)))
            }
        }
    }
}

fn init_updates_reader(mut receiver: Receiver<Box<Update>>) -> Receiver<NewUpdate> {
    let (sx, rx) = mpsc::channel(20);

    tokio::spawn(async move {
        while let Some(update) = receiver.recv().await {
            let new_update = match update.as_ref() {
                Update::ChatPhoto(chat_photo) => None,
                Update::ChatTitle(chat_title) => None,
                Update::File(file) =>
                match file.file().local().is_downloading_completed() {
                    false => None,
                    true => {
                        Some(NewUpdate::File(File{
                            local_path: Some(file.file().local().path().clone()),
                            remote_file: file.file().id(),
                            remote_id: file.file().remote().unique_id().clone(),
                    }))
                }},
                Update::MessageContent(content) => None,
                Update::NewChat(new_chat) => None,
                Update::NewMessage(new_message) => {
                    match new_message.message().is_channel_post() {
                        false => None,
                        true => {
                            let parsed = parsers::parse_message_content(new_message.message().content());
                            match parsed {
                                None => None,
                                Some((content, file)) => {
                                    if let Some(file) = file {
                                        if let Err(err) = sx
                                            .send_timeout(NewUpdate::File(file), SEND_UPDATE_TIMEOUT)
                                            .await
                                        {
                                            log::error!("cannot send new file update");
                                        }
                                    }
                                    Some(NewUpdate::Post(Post {
                                        title: None,
                                        link: "".to_string(),
                                        telegram_id: new_message.message().id(),
                                        pub_date: new_message.message().date(),
                                        content: content.unwrap_or_default(),
                                        chat_id: new_message.message().chat_id(),
                                    }))
                                }
                            }
                        }
                    }
                }
                Update::Poll(poll) => None,
                _ => None,
            };
            if let Some(new_update) = new_update {
                if let Err(err) = sx.send_timeout(new_update, SEND_UPDATE_TIMEOUT).await {
                    log::error!("cannot send new update");
                }
            }
        }
    });

    rx
}

#[derive(Clone, Debug)]
struct AuthHandler {
    encryption_key: String,
    phone_number: String,
}

impl AuthHandler {
    pub fn new(encryption_key: &str, phone_number: &str) -> Self {
        Self {
            encryption_key: encryption_key.to_string(),
            phone_number: phone_number.to_string(),
        }
    }

    fn wait_input() -> String {
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => input.trim().to_string(),
            Err(e) => panic!("Can not get input value: {:?}", e),
        }
    }
}

#[async_trait::async_trait]
impl AuthStateHandler for AuthHandler {
    async fn handle_other_device_confirmation(
        &self,
        _: &AuthorizationStateWaitOtherDeviceConfirmation,
    ) {
        panic!("other device confirmation not supported")
    }

    async fn handle_wait_code(&self, _: &AuthorizationStateWaitCode) -> String {
        eprintln!("wait for auth code");
        AuthHandler::wait_input()
    }

    async fn handle_encryption_key(&self, _: &AuthorizationStateWaitEncryptionKey) -> String {
        self.encryption_key.to_string()
    }

    async fn handle_wait_password(&self, _: &AuthorizationStateWaitPassword) -> String {
        panic!("password not supported")
    }

    async fn handle_wait_phone_number(&self, _: &AuthorizationStateWaitPhoneNumber) -> String {
        self.phone_number.to_string()
    }

    async fn handle_wait_registration(
        &self,
        _: &AuthorizationStateWaitRegistration,
    ) -> (String, String) {
        panic!("registration not supported")
    }
}


fn new_channel(chat: Chat, channel_name: &str) -> NewChannel {
    NewChannel {
        title: chat.title().clone(),
        telegram_id: chat.id(),
        username: channel_name.to_string(),
    }
}

fn new_file(file: &TgFile, post_id: i64) -> File {
    File {
        local_path: Some(file.local().path().as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        remote_file: file.id(),
        remote_id: file.remote().unique_id().clone(),
        telegram_post_id: post_id,
    }

}
fn is_channel(chat: &Chat) -> bool {
    match chat.type_() {
        ChatType::_Default => false,
        ChatType::BasicGroup(g) => false,
        ChatType::Private(_) => false,
        ChatType::Secret(_) => false,
        ChatType::Supergroup(sg) => {
            sg.is_channel()
        }
    }
}