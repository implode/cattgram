use worker::*;

mod handlers;
mod scraper;
mod templates;
mod utils;

fn embed_handler() -> impl Fn(Request, RouteContext<()>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response>>>> {
    |req, ctx| Box::pin(async move { handlers::embed::handle(req, ctx).await })
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    // Strip trailing slash (except root) and redirect-internally by rewriting
    let url = req.url()?;
    let path = url.path().to_string();

    if path.len() > 1 && path.ends_with('/') {
        let trimmed = path.trim_end_matches('/');
        let mut new_url = url.clone();
        new_url.set_path(trimmed);
        let new_req = Request::new_with_init(
            new_url.as_str(),
            &RequestInit {
                method: req.method(),
                headers: req.headers().clone(),
                ..Default::default()
            },
        )?;
        let router = build_router();
        return router.run(new_req, env).await;
    }

    let router = build_router();
    router.run(req, env).await
}

fn build_router() -> Router<'static, ()> {
    Router::new()
        .get("/", handlers::home::handle)
        .get_async("/p/:postID", embed_handler())
        .get_async("/p/:postID/:extra", embed_handler())
        .get_async("/:username/p/:postID", embed_handler())
        .get_async("/tv/:postID", embed_handler())
        .get_async("/reel/:postID", embed_handler())
        .get_async("/reels/:postID", embed_handler())
        .get_async("/stories/:username/:storyID", embed_handler())
        .get_async("/images/:postID/:mediaNum", |req, ctx| async move {
            handlers::media::images(req, ctx).await
        })
        .get_async("/videos/:postID/:mediaNum", |req, ctx| async move {
            handlers::media::videos(req, ctx).await
        })
        .get_async("/oembed", |req, ctx| async move {
            handlers::oembed::handle(req, ctx).await
        })
}
