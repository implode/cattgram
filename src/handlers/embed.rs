use url::Url;
use worker::*;

use crate::scraper::fetch_post_data;
use crate::templates::embed_html::render_embed;
use crate::utils::bot_detect::is_bot;
use crate::utils::instagram::{extract_post_id, mediaid_to_code};

/// Redirect to the original Instagram post.
fn redirect_to_instagram(post_id: &str) -> Result<Response> {
    let url = format!("https://www.instagram.com/p/{}/", post_id);
    Response::redirect(Url::parse(&url).map_err(|e| Error::RustError(e.to_string()))?)
}

/// Resolves a numeric story ID to a shortcode, or returns the input unchanged.
fn resolve_post_id(raw: &str) -> String {
    if raw.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(numeric_id) = raw.parse::<u64>() {
            return mediaid_to_code(numeric_id);
        }
    }
    raw.to_string()
}

/// Extracts the `img_index` query parameter (1-based) from a URL.
fn parse_img_index(url: &Url) -> Option<usize> {
    url.query_pairs()
        .find(|(k, _)| k == "img_index")
        .and_then(|(_, v)| v.parse::<usize>().ok())
        .filter(|&n| n >= 1)
}

/// Returns `true` if the `direct` query parameter is set to "true".
fn is_direct(url: &Url) -> bool {
    url.query_pairs()
        .any(|(k, v)| k == "direct" && v == "true")
}

/// Maximum number of redirects to follow when resolving share URLs.
const MAX_REDIRECTS: u8 = 5;

/// Follows a share URL redirect chain to extract the real post ID.
///
/// Uses `RequestRedirect::Manual` to intercept 3xx responses and read the
/// `Location` header. Follows up to `MAX_REDIRECTS` hops.
async fn resolve_share_url(share_path: &str) -> Result<Option<String>> {
    let mut current_url = format!("https://www.instagram.com/{}", share_path);

    for _ in 0..MAX_REDIRECTS {
        let headers = Headers::new();
        headers.set("User-Agent", "curl/8.0")?;

        let mut init = RequestInit::new();
        init.with_method(Method::Get)
            .with_headers(headers)
            .with_redirect(RequestRedirect::Manual);

        let request = Request::new_with_init(&current_url, &init)?;
        let resp = Fetch::Request(request).send().await?;

        let status = resp.status_code();
        if (300..400).contains(&status) {
            if let Some(location) = resp.headers().get("Location")? {
                // Location may be relative or absolute
                if let Ok(resolved) = Url::parse(&location)
                    .or_else(|_| Url::parse(&current_url).and_then(|base| base.join(&location)))
                {
                    // Check if we can already extract a post ID from this URL
                    if let Some(post_id) = extract_post_id(resolved.path()) {
                        return Ok(Some(post_id));
                    }
                    current_url = resolved.to_string();
                    continue;
                }
            }
            break;
        }

        // Non-redirect response: try to extract post ID from the URL we landed on
        if let Ok(parsed) = Url::parse(&current_url) {
            return Ok(extract_post_id(parsed.path()));
        }
        break;
    }

    Ok(None)
}

pub async fn handle(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    // 1. Extract post ID from route params
    let raw_post_id = ctx
        .param("postID")
        .or_else(|| ctx.param("storyID"))
        .cloned()
        .unwrap_or_default();

    if raw_post_id.is_empty() {
        return redirect_to_instagram("");
    }

    // 2. Resolve numeric story IDs to shortcodes
    let mut post_id = resolve_post_id(&raw_post_id);

    // 3. Parse query params
    let req_url = req.url().map_err(|e| Error::RustError(e.to_string()))?;
    let img_index = parse_img_index(&req_url);
    let direct = is_direct(&req_url);

    // 4. Handle share URLs (post_id starts with "share")
    if post_id.starts_with("share") {
        // The route would match /p/share/... so the param would be "share"
        // and the extra segment holds the share ID. Reconstruct the share path.
        let extra = ctx.param("extra").cloned().unwrap_or_default();
        let share_path = if extra.is_empty() {
            format!("share/{}", post_id.trim_start_matches("share/").trim_start_matches("share"))
        } else {
            format!("share/{}", extra)
        };

        match resolve_share_url(&share_path).await {
            Ok(Some(resolved)) => post_id = resolved,
            _ => return redirect_to_instagram(&post_id),
        }
    }

    // 5. Bot detection: non-bots get redirected to Instagram
    let ua = req
        .headers()
        .get("User-Agent")
        .unwrap_or(None)
        .unwrap_or_default();

    console_log!("[embed] post_id={} ua={} is_bot={}", post_id, ua, is_bot(&ua));

    if !is_bot(&ua) {
        return redirect_to_instagram(&post_id);
    }

    // 6. Fetch Instagram data
    let data = match fetch_post_data(&post_id, &ctx.env).await {
        Ok(Some(data)) => {
            console_log!("[embed] got data: username={} media_count={}", data.username, data.media.len());
            data
        }
        Ok(None) => {
            console_log!("[embed] no data found, redirecting to instagram");
            return redirect_to_instagram(&post_id);
        }
        Err(e) => {
            console_log!("[embed] fetch error: {:?}", e);
            return redirect_to_instagram(&post_id);
        }
    };

    // 7. Direct media redirect
    if direct {
        let media_index = img_index
            .map(|i| i.saturating_sub(1))
            .unwrap_or(0)
            .min(data.media.len().saturating_sub(1));

        if let Some(media) = data.media.get(media_index) {
            let redirect_url =
                Url::parse(&media.url).map_err(|e| Error::RustError(e.to_string()))?;
            return Response::redirect(redirect_url);
        }

        return redirect_to_instagram(&post_id);
    }

    // 8. Generate embed HTML
    let host = req_url.host_str().unwrap_or("cattgram.com").to_string();
    let html = render_embed(&data, &host, img_index);
    console_log!("[embed] returning HTML, first 1000 chars: {}", &html[..html.len().min(1000)]);
    Response::from_html(html)
}
