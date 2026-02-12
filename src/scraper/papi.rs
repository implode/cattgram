use worker::*;

use super::proxy::proxy_fetch;
use super::types::{InstaData, Media, MediaType};
use crate::utils::instagram::code_to_mediaid;

/// Instagram mobile app user-agent (PAPI is the mobile/private API)
const IG_MOBILE_UA: &str = "Instagram 317.0.0.34.109 Android (31/12; 420dpi; 1080x2400; samsung; SM-G991B; o1s; exynos2100; en_US; 562530885)";

/// Fetches post data from Instagram's Private API (mobile API).
///
/// Uses `https://i.instagram.com/api/v1/media/{media_id}/info/` which
/// requires a valid session cookie (set as `IG_COOKIE` secret).
/// Tries direct fetch first, then falls back to proxy.
pub async fn fetch_papi(post_id: &str, env: &Env) -> Result<Option<InstaData>> {
    let raw_cookie = match env.secret("IG_COOKIE") {
        Ok(c) => c.to_string(),
        Err(_) => {
            console_log!("[papi] no IG_COOKIE secret configured, skipping");
            return Ok(None);
        }
    };

    // URL-decode the cookie in case wrangler stored it encoded
    let decoded_cookie = raw_cookie
        .replace("%3A", ":")
        .replace("%3a", ":");

    // Auto-wrap raw session ID values with "sessionid=" prefix
    let cookie = if decoded_cookie.contains('=') {
        decoded_cookie.clone()
    } else {
        format!("sessionid={}", decoded_cookie)
    };

    // Extract user ID from sessionid value and add ds_user_id cookie
    // Session format: sessionid={user_id}:{token}:{version}:{hash}
    let full_cookie = if let Some(sid_val) = cookie.strip_prefix("sessionid=") {
        if let Some(user_id) = sid_val.split(':').next() {
            format!("{}; ds_user_id={}", cookie, user_id)
        } else {
            cookie.clone()
        }
    } else {
        cookie.clone()
    };
    console_log!("[papi] cookie starts with: {}", &full_cookie[..full_cookie.len().min(50)]);

    // Convert shortcode to numeric media ID
    let media_id = match code_to_mediaid(post_id) {
        Some(id) => id,
        None => {
            console_log!("[papi] failed to convert shortcode {} to media ID", post_id);
            return Ok(None);
        }
    };

    let url = format!("https://i.instagram.com/api/v1/media/{media_id}/info/");
    console_log!("[papi] fetching media_id={} for shortcode={}", media_id, post_id);

    // Try direct fetch first
    let text = match papi_direct_fetch(&url, &full_cookie).await {
        Ok(t) if !t.contains("not-logged-in") && !t.contains("Page Not Found") => {
            console_log!("[papi] direct fetch succeeded");
            t
        }
        Ok(_) => {
            console_log!("[papi] direct fetch returned login/404, trying via proxy");
            // Fall back to proxy
            match papi_proxy_fetch(&url, &full_cookie, env).await {
                Ok(t) => t,
                Err(e) => {
                    console_log!("[papi] proxy fetch error: {:?}", e);
                    return Ok(None);
                }
            }
        }
        Err(e) => {
            console_log!("[papi] direct fetch error: {:?}, trying proxy", e);
            match papi_proxy_fetch(&url, &full_cookie, env).await {
                Ok(t) => t,
                Err(e) => {
                    console_log!("[papi] proxy fetch error: {:?}", e);
                    return Ok(None);
                }
            }
        }
    };

    console_log!("[papi] response_len={} first_200={}", text.len(), &text[..text.len().min(200)]);

    let json: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            console_log!("[papi] JSON parse error: {}", e);
            return Ok(None);
        }
    };

    let items = match json.get("items").and_then(|i| i.as_array()) {
        Some(items) if !items.is_empty() => items,
        _ => {
            console_log!("[papi] no items in response");
            return Ok(None);
        }
    };

    let item = &items[0];
    parse_papi_item(item, post_id)
}

/// Direct PAPI fetch from CF Worker.
async fn papi_direct_fetch(url: &str, cookie: &str) -> Result<String> {
    let headers = build_papi_headers(cookie)?;

    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);

    let request = Request::new_with_init(url, &init)?;
    let mut resp = Fetch::Request(request).send().await?;

    let status = resp.status_code();
    let text = resp.text().await?;
    console_log!("[papi] direct status={} len={} body={}", status, text.len(), &text[..text.len().min(500)]);

    if status != 200 {
        return Err(Error::RustError(format!("PAPI direct returned {}", status)));
    }
    Ok(text)
}

/// PAPI fetch via Bright Data proxy (passes cookie in headers).
async fn papi_proxy_fetch(url: &str, cookie: &str, env: &Env) -> Result<String> {
    let headers = build_papi_headers(cookie)?;

    let mut resp = proxy_fetch(url, Method::Get, headers, None, env).await?;

    let status = resp.status_code();
    let text = resp.text().await?;
    console_log!("[papi] proxy status={} len={}", status, text.len());

    if status != 200 {
        return Err(Error::RustError(format!("PAPI proxy returned {}", status)));
    }
    Ok(text)
}

fn build_papi_headers(cookie: &str) -> Result<Headers> {
    let headers = Headers::new();
    headers.set("User-Agent", IG_MOBILE_UA)?;
    headers.set("Accept", "*/*")?;
    headers.set("Accept-Language", "en-US,en;q=0.9")?;
    headers.set("X-Ig-App-Id", "567067343352427")?; // Instagram Android app ID
    headers.set("Cookie", cookie)?;
    Ok(headers)
}

/// Parses a single media item from the PAPI response.
fn parse_papi_item(item: &serde_json::Value, post_id: &str) -> Result<Option<InstaData>> {
    let username = item
        .get("user")
        .and_then(|u| u.get("username"))
        .and_then(|u| u.as_str())
        .unwrap_or("unknown")
        .to_string();

    let caption = item
        .get("caption")
        .and_then(|c| c.get("text"))
        .and_then(|t| t.as_str())
        .map(String::from);

    let like_count = item.get("like_count").and_then(|l| l.as_u64());
    let comment_count = item.get("comment_count").and_then(|c| c.as_u64());
    let timestamp = item.get("taken_at").and_then(|t| t.as_u64()).unwrap_or(0);

    // Check for carousel (multiple media items)
    let media_items = if let Some(carousel) = item.get("carousel_media").and_then(|c| c.as_array()) {
        carousel.iter().filter_map(|m| parse_papi_media(m)).collect()
    } else {
        // Single media item
        match parse_papi_media(item) {
            Some(m) => vec![m],
            None => Vec::new(),
        }
    };

    let is_video = item.get("video_versions").is_some()
        || media_items.iter().any(|m| m.media_type == MediaType::Video);

    let video_view_count = item.get("view_count").and_then(|v| v.as_u64());

    console_log!("[papi] parsed: username={} media_count={} is_video={}", username, media_items.len(), is_video);

    Ok(Some(InstaData {
        post_id: post_id.to_string(),
        username,
        caption,
        media: media_items,
        like_count,
        comment_count,
        is_video,
        video_view_count,
        timestamp,
    }))
}

/// Parses a single media node from PAPI response format.
fn parse_papi_media(node: &serde_json::Value) -> Option<Media> {
    // Video: video_versions array has URL
    if let Some(video_versions) = node.get("video_versions").and_then(|v| v.as_array()) {
        if let Some(best) = video_versions.first() {
            let url = best.get("url").and_then(|u| u.as_str()).unwrap_or_default().to_string();
            let width = best.get("width").and_then(|w| w.as_u64()).map(|w| w as u32);
            let height = best.get("height").and_then(|h| h.as_u64()).map(|h| h as u32);
            let thumbnail_url = node
                .get("image_versions2")
                .and_then(|i| i.get("candidates"))
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|img| img.get("url"))
                .and_then(|u| u.as_str())
                .map(String::from);
            return Some(Media {
                media_type: MediaType::Video,
                url,
                thumbnail_url,
                width,
                height,
            });
        }
    }

    // Image: image_versions2.candidates array
    let candidates = node
        .get("image_versions2")
        .and_then(|i| i.get("candidates"))
        .and_then(|c| c.as_array())?;

    let best = candidates.first()?;
    let url = best.get("url").and_then(|u| u.as_str()).unwrap_or_default().to_string();
    let width = best.get("width").and_then(|w| w.as_u64()).map(|w| w as u32);
    let height = best.get("height").and_then(|h| h.as_u64()).map(|h| h as u32);

    Some(Media {
        media_type: MediaType::Image,
        url,
        thumbnail_url: None,
        width,
        height,
    })
}
