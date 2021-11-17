mod app;
mod db;
pub mod models;
mod server;
mod settings;
mod telegram;

extern crate time;

use crate::app::App;
use db::DbService;
use settings::Settings;
use telegram::TelegramService;

#[tokio::main]
async fn main() {
    env_logger::init();
    let settings = Settings::new().expect("can't get config");
    log::info!("initializing database");
    let db = DbService::new(settings.db.path.as_str())
        .await
        .expect("can't connect to db");

    let telegram = TelegramService::new(
        settings.telegram.api_hash,
        settings.telegram.api_id,
        settings.telegram.phone,
    );

    let app = App::new(telegram, db);
    app.start().await.expect("cannot start application");

    app.synchronize_channels().await.expect("cannot synchronize channels");
    app.synchronize_files().await.expect("cannot synchronize files");

    log::info!("starting web server");
    server::run_server(app).await;
}
