use worker::*;

use super::proxy::proxy_fetch;
use super::types::{InstaData, Media, MediaType};

const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Returns true if the embed page HTML indicates a video that can't be played inline.
pub fn is_video_blocked(html: &str) -> bool {
    html.contains("WatchOnInstagram") || html.contains("EmbeddedMediaVideo")
}

pub async fn fetch_embed_page(post_id: &str, env: &Env) -> worker::Result<Option<(InstaData, bool)>> {
    let url_str = format!("https://www.instagram.com/p/{post_id}/embed/captioned/?_fb_noscript=1");

    let headers = Headers::new();
    headers.set("User-Agent", CHROME_UA)?;
    headers.set("Accept", "text/html,application/xhtml+xml")?;
    headers.set("Accept-Language", "en-US,en;q=0.9")?;

    // Pass session cookie through proxy if available — helps bypass login walls
    if let Ok(cookie_secret) = env.secret("IG_COOKIE") {
        let raw = cookie_secret.to_string().replace("%3A", ":").replace("%3a", ":");
        let cookie = if raw.contains('=') { raw } else { format!("sessionid={}", raw) };
        headers.set("Cookie", &cookie)?;
    }

    let mut resp = proxy_fetch(&url_str, Method::Get, headers, None, env).await?;

    let status = resp.status_code();
    let html = resp.text().await?;
    console_log!("[embed_page] status={} html_len={} for {}", status, html.len(), post_id);

    if status != 200 {
        console_log!("[embed_page] non-200 response, first 500 chars: {}", &html[..html.len().min(500)]);
        return Ok(None);
    }

    let video_blocked = is_video_blocked(&html);
    console_log!("[embed_page] video_blocked={} for {}", video_blocked, post_id);

    // Try structured JSON extraction first
    if let Some(data) = extract_from_json(&html, post_id) {
        console_log!("[embed_page] JSON extraction succeeded for {}", post_id);
        return Ok(Some((data, video_blocked)));
    }
    console_log!("[embed_page] JSON extraction failed, trying contextJSON for {}", post_id);

    // Try contextJSON extraction (double-encoded JSON with gql_data)
    if let Some(data) = extract_from_context_json(&html, post_id) {
        console_log!("[embed_page] contextJSON extraction succeeded for {}", post_id);
        return Ok(Some((data, video_blocked)));
    }
    console_log!("[embed_page] contextJSON failed, trying HTML fallback for {}", post_id);

    if let Some(data) = extract_from_html(&html, post_id) {
        console_log!("[embed_page] HTML extraction succeeded for {}. media_urls: {:?}",
            post_id, data.media.iter().map(|m| &m.url).collect::<Vec<_>>());
        return Ok(Some((data, video_blocked)));
    }

    console_log!("[embed_page] all extraction failed for {}. Has shortcode_media: {} Has EmbeddedMedia: {} Has login: {} first_500: {}",
        post_id,
        html.contains("shortcode_media"),
        html.contains("EmbeddedMedia"),
        html.contains("login") || html.contains("Login"),
        &html[..html.len().min(500)]);
    Ok(None)
}

/// Extracts post data from the embedded `shortcode_media` JSON blob in the page.
fn extract_from_json(html: &str, post_id: &str) -> Option<InstaData> {
    let json_obj = extract_shortcode_media_json(html)?;
    let media_obj: serde_json::Value = serde_json::from_str(&json_obj).ok()?;
    parse_shortcode_media(&media_obj, post_id)
}

/// Extracts post data from the double-encoded `contextJSON` in the embed page.
///
/// Instagram embeds sometimes include a `"contextJSON":"..."` field that contains
/// a double-encoded JSON string. Inside it, `gql_data` has the same structure as
/// `shortcode_media`.
fn extract_from_context_json(html: &str, post_id: &str) -> Option<InstaData> {
    let needle = "\"contextJSON\":\"";
    let start = html.find(needle)?;
    let str_start = start + needle.len() - 1; // include the opening quote

    // Walk through the string to find the unescaped closing quote
    let bytes = html.as_bytes();
    let mut i = str_start + 1; // skip opening quote
    let mut escape = false;
    while i < bytes.len() {
        if escape {
            escape = false;
            i += 1;
            continue;
        }
        if bytes[i] == b'\\' {
            escape = true;
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            // Found the closing quote
            break;
        }
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }

    // Parse the JSON string (with quotes) to unescape it
    let json_str = &html[str_start..=i];
    let inner_str: String = serde_json::from_str(json_str).ok()?;

    // Parse the inner string as JSON
    let context: serde_json::Value = serde_json::from_str(&inner_str).ok()?;

    // Extract gql_data which contains shortcode_media structure
    let gql_data = context.get("gql_data")?;
    let media = gql_data.get("shortcode_media")
        .or_else(|| gql_data.get("xdt_shortcode_media"))?;

    console_log!("[embed_page] contextJSON found gql_data for {}", post_id);
    parse_shortcode_media(media, post_id)
}

/// Locates `"shortcode_media":` in the HTML and extracts the balanced JSON object.
fn extract_shortcode_media_json(html: &str) -> Option<String> {
    let needle = "\"shortcode_media\":";
    let start = html.find(needle)?;
    let json_start = start + needle.len();

    // Find the opening brace
    let rest = &html[json_start..];
    let brace_offset = rest.find('{')?;
    let obj_start = json_start + brace_offset;

    // Track brace depth to find the matching closing brace
    let mut depth: u32 = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in html[obj_start..].char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if ch == '\\' && in_string {
            escape_next = true;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            continue;
        }

        if in_string {
            continue;
        }

        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(html[obj_start..obj_start + i + 1].to_string());
                }
            }
            _ => {}
        }
    }

    None
}

/// Parses a `shortcode_media` JSON value into `InstaData`.
pub fn parse_shortcode_media(media: &serde_json::Value, post_id: &str) -> Option<InstaData> {
    let username = media
        .get("owner")?
        .get("username")?
        .as_str()?
        .to_string();

    let caption = media
        .get("edge_media_to_caption")
        .and_then(|c| c.get("edges"))
        .and_then(|e| e.as_array())
        .and_then(|arr| arr.first())
        .and_then(|edge| edge.get("node"))
        .and_then(|node| node.get("text"))
        .and_then(|t| t.as_str())
        .map(String::from);

    let is_video = media.get("is_video").and_then(|v| v.as_bool()).unwrap_or(false);
    let timestamp = media
        .get("taken_at_timestamp")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);

    let like_count = media
        .get("edge_media_preview_like")
        .and_then(|l| l.get("count"))
        .and_then(|c| c.as_u64());

    let comment_count = media
        .get("edge_media_to_comment")
        .and_then(|l| l.get("count"))
        .and_then(|c| c.as_u64());

    let video_view_count = media
        .get("video_view_count")
        .and_then(|v| v.as_u64());

    let media_items = build_media_list(media);

    Some(InstaData {
        post_id: post_id.to_string(),
        username,
        caption,
        media: media_items,
        like_count,
        comment_count,
        is_video,
        video_view_count,
        timestamp,
    })
}

/// Builds a `Vec<Media>` from the shortcode_media JSON, handling carousels and single posts.
fn build_media_list(media: &serde_json::Value) -> Vec<Media> {
    // Carousel: edge_sidecar_to_children contains multiple items
    if let Some(children) = media
        .get("edge_sidecar_to_children")
        .and_then(|c| c.get("edges"))
        .and_then(|e| e.as_array())
    {
        return children
            .iter()
            .filter_map(|edge| {
                let node = edge.get("node")?;
                Some(media_from_node(node))
            })
            .collect();
    }

    // Single post
    vec![media_from_node(media)]
}

/// Converts a single media node into a `Media` struct.
fn media_from_node(node: &serde_json::Value) -> Media {
    let is_video = node.get("is_video").and_then(|v| v.as_bool()).unwrap_or(false);

    let (media_type, url, thumbnail_url) = if is_video {
        let video_url = node
            .get("video_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let thumb = node.get("display_url").and_then(|v| v.as_str()).map(String::from);
        (MediaType::Video, video_url, thumb)
    } else {
        let display_url = node
            .get("display_url")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        (MediaType::Image, display_url, None)
    };

    let width = node
        .get("dimensions")
        .and_then(|d| d.get("width"))
        .and_then(|w| w.as_u64())
        .map(|w| w as u32);

    let height = node
        .get("dimensions")
        .and_then(|d| d.get("height"))
        .and_then(|h| h.as_u64())
        .map(|h| h as u32);

    Media {
        media_type,
        url,
        thumbnail_url,
        width,
        height,
    }
}

/// Fallback: scrape basic info from the embed HTML markup when no JSON blob is found.
fn extract_from_html(html: &str, post_id: &str) -> Option<InstaData> {
    let image_url = extract_attr_from_class(html, "EmbeddedMediaImage", "src")?;
    let username = extract_text_from_class(html, "UsernameText").unwrap_or_else(|| "unknown".to_string());
    let caption = extract_caption_text(html);

    Some(InstaData {
        post_id: post_id.to_string(),
        username,
        caption,
        media: vec![Media {
            media_type: MediaType::Image,
            url: image_url,
            thumbnail_url: None,
            width: None,
            height: None,
        }],
        like_count: None,
        comment_count: None,
        is_video: false,
        video_view_count: None,
        timestamp: 0,
    })
}

/// Finds an element with the given class name and extracts a specific attribute value.
fn extract_attr_from_class(html: &str, class_name: &str, attr: &str) -> Option<String> {
    let class_pos = html.find(class_name)?;

    // Walk backwards to find the opening `<` of this tag
    let before = &html[..class_pos];
    let tag_start = before.rfind('<')?;

    // Search forward from the tag start for the target attribute
    let tag_region = &html[tag_start..];
    let tag_end = tag_region.find('>')?;
    let tag_str = &tag_region[..tag_end + 1];

    let attr_needle = format!("{attr}=\"");
    let attr_start = tag_str.find(&attr_needle)?;
    let value_start = attr_start + attr_needle.len();
    let value_end = tag_str[value_start..].find('"')?;

    let raw = &tag_str[value_start..value_start + value_end];
    Some(unescape_html_entities(raw))
}

/// Unescapes common HTML entities back to their raw characters.
fn unescape_html_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
}

/// Extracts the inner text content of the first element with the given class name.
fn extract_text_from_class(html: &str, class_name: &str) -> Option<String> {
    let class_pos = html.find(class_name)?;

    // Find the end of the opening tag
    let rest = &html[class_pos..];
    let tag_close = rest.find('>')?;
    let content_start = class_pos + tag_close + 1;

    // Find the next closing tag
    let content_rest = &html[content_start..];
    let next_tag = content_rest.find('<')?;

    let text = content_rest[..next_tag].trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Attempts to extract caption text from the embed page.
/// Looks for text content near the "CaptionUsername" class.
fn extract_caption_text(html: &str) -> Option<String> {
    let marker = "CaptionUsername";
    let pos = html.find(marker)?;

    // Skip past the CaptionUsername element to find sibling text
    let rest = &html[pos..];

    // Find the closing tag of the username span
    let first_close = rest.find("</")? ;
    let after_username = &rest[first_close..];

    // Skip the closing tag itself
    let after_tag = after_username.find('>')? + 1;
    let content = &after_username[after_tag..];

    // Grab text until the next HTML tag
    let next_tag = content.find('<').unwrap_or(content.len());
    let text = content[..next_tag].trim();

    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}
