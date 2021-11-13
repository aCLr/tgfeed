mod app;
mod db;
mod server;
mod settings;
mod telegram;

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
    log::info!("starting telegram service");
    telegram
        .start()
        .await
        .expect("can't start telegram service");
    log::info!("telegram service started");

    let app = App::new(telegram, db);
    log::info!("starting web server");
    server::run_server(app).await;
}
