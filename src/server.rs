use crate::app::App;

pub async fn run_server(app: App) -> anyhow::Result<()> {
    let routes = filters::channel(app);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}
mod filters {
    use super::handlers;
    use crate::app::App;
    use warp::Filter;

    pub fn channel(
        app: App,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path!("channel" / String)
            .and(warp::get())
            .and(with_app(app))
            .and_then(handlers::channel)
    }

    fn with_app(
        db: App,
    ) -> impl Filter<Extract = (App,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || db.clone())
    }
}

mod handlers {
    use crate::app::App;
    use warp::http::header::CONTENT_TYPE;
    use warp::http::StatusCode;
    use warp::Reply;

    pub async fn channel(
        channel_name: String,
        app: App,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        let response: warp::reply::Response;
        match app.get_posts_or_search(channel_name.as_str()).await {
            Ok(feed) => match feed {
                None => {
                    response = warp::reply::with_status("".to_string(), StatusCode::NOT_FOUND)
                        .into_response();
                }
                Some(feed) => {
                    response = warp::reply::with_header(
                        feed.to_string(),
                        CONTENT_TYPE,
                        "application/rss+xml",
                    )
                    .into_response()
                }
            },
            Err(err) => {
                response =
                    warp::reply::with_status(err.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
                        .into_response()
            }
        }
        Ok(response)
    }
}
