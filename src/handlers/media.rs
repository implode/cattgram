use url::Url;
use worker::*;

use crate::scraper::fetch_post_data;
use crate::scraper::types::MediaType;

/// Redirect to the original Instagram post.
fn redirect_to_instagram(post_id: &str) -> Result<Response> {
    let url = format!("https://www.instagram.com/p/{}/", post_id);
    Response::redirect(Url::parse(&url).map_err(|e| Error::RustError(e.to_string()))?)
}

/// Redirect to a media URL.
fn redirect_to_url(media_url: &str) -> Result<Response> {
    let parsed = Url::parse(media_url).map_err(|e| Error::RustError(e.to_string()))?;
    Response::redirect(parsed)
}

/// Extracts the `postID` and `mediaNum` (1-based) from route params.
fn extract_params(ctx: &RouteContext<()>) -> Option<(String, usize)> {
    let post_id = ctx.param("postID")?.to_string();
    let media_num: usize = ctx.param("mediaNum")?.parse().ok()?;
    if media_num >= 1 {
        Some((post_id, media_num))
    } else {
        None
    }
}

/// Direct image redirect handler.
///
/// Route: `/images/:postID/:mediaNum`
/// Fetches the post, selects the Nth media item (1-based), and redirects to its image URL.
pub async fn images(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (post_id, media_num) = match extract_params(&ctx) {
        Some(params) => params,
        None => return Response::error("Bad Request", 400),
    };

    let data = match fetch_post_data(&post_id, &ctx.env).await {
        Ok(Some(data)) => data,
        _ => return redirect_to_instagram(&post_id),
    };

    let index = media_num - 1;
    match data.media.get(index) {
        Some(media) if media.media_type == MediaType::Image => redirect_to_url(&media.url),
        Some(media) if media.thumbnail_url.is_some() => {
            // Video with a thumbnail: return the thumbnail as the "image"
            redirect_to_url(media.thumbnail_url.as_ref().unwrap())
        }
        _ => redirect_to_instagram(&post_id),
    }
}

/// Direct video redirect handler.
///
/// Route: `/videos/:postID/:mediaNum`
/// Fetches the post, selects the Nth media item (1-based), and redirects to its video URL.
pub async fn videos(_req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let (post_id, media_num) = match extract_params(&ctx) {
        Some(params) => params,
        None => return Response::error("Bad Request", 400),
    };

    let data = match fetch_post_data(&post_id, &ctx.env).await {
        Ok(Some(data)) => data,
        _ => return redirect_to_instagram(&post_id),
    };

    let index = media_num - 1;
    match data.media.get(index) {
        Some(media) if media.media_type == MediaType::Video => redirect_to_url(&media.url),
        _ => redirect_to_instagram(&post_id),
    }
}
