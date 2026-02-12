pub mod cache;
pub mod embed_page;
pub mod graphql;
pub mod papi;
pub mod proxy;
pub mod types;

use worker::*;

use self::cache::{get_cached, set_cached};
use self::embed_page::fetch_embed_page;
use self::graphql::fetch_graphql;
use self::papi::fetch_papi;
use self::types::InstaData;

/// Orchestrator: cache -> embed page -> graphql fallback
///
/// The embed page JSON extraction gives complete data (images + videos).
/// The embed page HTML fallback only gives thumbnails — never video URLs.
/// So when HTML fallback is used, we always try GraphQL for better data.
pub async fn fetch_post_data(post_id: &str, env: &Env) -> Result<Option<InstaData>> {
    console_log!("[scraper] fetching post_id={}", post_id);

    // 1. Check cache
    match get_cached(post_id, env).await {
        Ok(Some(cached)) => {
            console_log!("[scraper] cache HIT for {}", post_id);
            return Ok(Some(cached));
        }
        Ok(None) => console_log!("[scraper] cache MISS for {}", post_id),
        Err(e) => console_log!("[scraper] cache error: {:?}", e),
    }

    // 2. Try embed page
    let mut embed_fallback: Option<InstaData> = None;

    match fetch_embed_page(post_id, env).await {
        Ok(Some((data, video_blocked))) => {
            // JSON extraction gets full data (including video URLs) — use directly
            // HTML fallback only gets thumbnails — always try GraphQL for better data
            let json_extraction = data.is_video || data.media.iter().any(|m| m.media_type == types::MediaType::Video);
            let has_video_url = data.media.iter().any(|m| {
                m.media_type == types::MediaType::Video && !m.url.is_empty()
            });

            if !video_blocked && (json_extraction || has_video_url || !data.media.is_empty()) {
                // Check if this looks like complete data (JSON extraction) vs HTML fallback (thumbnail only)
                // HTML fallback always produces Image type with no dimensions
                let is_html_fallback = data.media.len() == 1
                    && data.media[0].media_type == types::MediaType::Image
                    && data.media[0].width.is_none()
                    && data.media[0].height.is_none();

                if !is_html_fallback {
                    console_log!("[scraper] embed page JSON data complete for {} (username={})", post_id, data.username);
                    let _ = set_cached(post_id, &data, env).await;
                    return Ok(Some(data));
                }

                console_log!("[scraper] embed page HTML fallback for {} — trying GraphQL for richer data", post_id);
                embed_fallback = Some(data);
            } else if video_blocked {
                console_log!("[scraper] video blocked in embed for {} — trying GraphQL", post_id);
                embed_fallback = Some(data);
            }
        }
        Ok(None) => console_log!("[scraper] embed page returned None for {}", post_id),
        Err(e) => console_log!("[scraper] embed page ERROR for {}: {:?}", post_id, e),
    }

    // 3. GraphQL — try for videos, incomplete data, or when embed page failed entirely
    let doc_id = env.var("GRAPHQL_DOC_ID")
        .map(|v| v.to_string())
        .unwrap_or_else(|_| "25531498899829322".to_string());
    console_log!("[scraper] trying graphql for {} with doc_id={}", post_id, doc_id);

    match fetch_graphql(post_id, &doc_id, env).await {
        Ok(Some(data)) => {
            console_log!("[scraper] graphql SUCCESS for {} (username={}, media_count={}, is_video={})",
                post_id, data.username, data.media.len(), data.is_video);
            let _ = set_cached(post_id, &data, env).await;
            return Ok(Some(data));
        }
        Ok(None) => console_log!("[scraper] graphql returned None for {}", post_id),
        Err(e) => console_log!("[scraper] graphql ERROR for {}: {:?}", post_id, e),
    }

    // 4. Try Instagram Private API (requires IG_COOKIE secret)
    console_log!("[scraper] trying PAPI for {}", post_id);
    match fetch_papi(post_id, env).await {
        Ok(Some(data)) => {
            console_log!("[scraper] PAPI SUCCESS for {} (username={}, media_count={}, is_video={})",
                post_id, data.username, data.media.len(), data.is_video);
            let _ = set_cached(post_id, &data, env).await;
            return Ok(Some(data));
        }
        Ok(None) => console_log!("[scraper] PAPI returned None for {}", post_id),
        Err(e) => console_log!("[scraper] PAPI ERROR for {}: {:?}", post_id, e),
    }

    // 5. Fall back to embed page thumbnail if everything else failed
    if let Some(data) = embed_fallback {
        console_log!("[scraper] falling back to embed page thumbnail for {}", post_id);
        let _ = set_cached(post_id, &data, env).await;
        return Ok(Some(data));
    }

    console_log!("[scraper] all methods failed for {}", post_id);
    Ok(None)
}
