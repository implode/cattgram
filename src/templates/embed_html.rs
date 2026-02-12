use crate::scraper::types::{InstaData, MediaType};
use crate::utils::escape::escape_html;

/// Truncates a string to `max_len` characters, appending "..." if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut end = max_len;
        // Avoid splitting a multi-byte character
        while !s.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}

/// Formats a number with comma separators (e.g. 1234567 -> "1,234,567").
fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result
}

/// Builds the stats suffix for the og:title tag.
fn build_stats_suffix(data: &InstaData, media_count: usize, img_index: Option<usize>) -> String {
    let mut parts = Vec::new();

    if data.is_video {
        if let Some(views) = data.video_view_count {
            parts.push(format!("{} views", format_number(views)));
        }
    }

    if let Some(likes) = data.like_count {
        parts.push(format!("{} likes", format_number(likes)));
    }

    if let Some(comments) = data.comment_count {
        parts.push(format!("{} comments", format_number(comments)));
    }

    if media_count > 1 {
        let idx = img_index.unwrap_or(1);
        parts.push(format!("Slide {}/{}", idx, media_count));
    }

    if parts.is_empty() {
        String::new()
    } else {
        format!(" | {}", parts.join(", "))
    }
}

/// Appends a `<meta>` tag to the HTML buffer.
fn push_meta(buf: &mut String, attr: &str, name: &str, content: &str) {
    buf.push_str(&format!(
        "<meta {}=\"{}\" content=\"{}\">\n",
        attr, name, content,
    ));
}

/// Renders a full HTML embed page with OpenGraph and Twitter Card meta tags.
///
/// `img_index` is 1-based. If `None` or out of range, defaults to the first media item.
pub fn render_embed(data: &InstaData, host: &str, img_index: Option<usize>) -> String {
    let media_count = data.media.len();

    // Resolve the target media item (img_index is 1-based)
    let resolved_index = img_index
        .map(|i| i.saturating_sub(1))
        .unwrap_or(0)
        .min(media_count.saturating_sub(1));

    let media_item = data.media.get(resolved_index);

    let username = escape_html(&data.username);
    let post_id = escape_html(&data.post_id);

    let caption = data
        .caption
        .as_deref()
        .map(|c| escape_html(&truncate(c, 300)))
        .unwrap_or_default();

    let stats_suffix = escape_html(&build_stats_suffix(data, media_count, img_index));
    let title = format!("@{}{}", username, stats_suffix);

    let instagram_url = format!("https://www.instagram.com/p/{}/", post_id);
    let oembed_url = format!(
        "https://{}/oembed?text=@{}&amp;url=https://instagram.com/p/{}",
        escape_html(host),
        username,
        post_id,
    );

    let mut html = String::with_capacity(4096);

    html.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");

    // Core OG tags
    push_meta(&mut html, "property", "theme-color", "#E1306C");
    push_meta(&mut html, "property", "og:site_name", "Cattgram");
    push_meta(&mut html, "property", "og:title", &title);
    push_meta(&mut html, "property", "og:description", &caption);
    push_meta(&mut html, "property", "og:url", &instagram_url);

    // Media-specific tags
    if let Some(media) = media_item {
        let width_str = media.width.unwrap_or(0).to_string();
        let height_str = media.height.unwrap_or(0).to_string();

        match media.media_type {
            MediaType::Image => {
                let image_url = escape_html(&media.url);
                push_meta(&mut html, "property", "og:image", &image_url);
                push_meta(&mut html, "property", "og:image:width", &width_str);
                push_meta(&mut html, "property", "og:image:height", &height_str);
                push_meta(&mut html, "name", "twitter:card", "summary_large_image");
                push_meta(&mut html, "name", "twitter:image", &image_url);
            }
            MediaType::Video => {
                let video_url = escape_html(&media.url);
                push_meta(&mut html, "property", "og:video", &video_url);
                push_meta(&mut html, "property", "og:video:type", "video/mp4");
                push_meta(&mut html, "property", "og:video:width", &width_str);
                push_meta(&mut html, "property", "og:video:height", &height_str);
                push_meta(&mut html, "name", "twitter:card", "player");
                push_meta(&mut html, "name", "twitter:player:stream", &video_url);
                push_meta(
                    &mut html,
                    "name",
                    "twitter:player:stream:content_type",
                    "video/mp4",
                );

                if let Some(ref thumbnail) = media.thumbnail_url {
                    push_meta(&mut html, "property", "og:image", &escape_html(thumbnail));
                }
            }
        }
    }

    html.push_str(&format!(
        "<link rel=\"alternate\" href=\"{}\" type=\"application/json+oembed\">\n",
        oembed_url,
    ));
    html.push_str(&format!(
        "<meta http-equiv=\"refresh\" content=\"0;url={}\">\n",
        instagram_url,
    ));
    html.push_str("<title>Cattgram</title>\n</head>\n<body>\n");
    html.push_str("<p>Redirecting to Instagram...</p>\n");
    html.push_str("</body>\n</html>");

    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scraper::types::{InstaData, Media, MediaType};

    fn sample_image_data() -> InstaData {
        InstaData {
            post_id: "ABC123".to_string(),
            username: "testuser".to_string(),
            caption: Some("Hello world!".to_string()),
            media: vec![Media {
                media_type: MediaType::Image,
                url: "https://cdn.example.com/image.jpg".to_string(),
                thumbnail_url: None,
                width: Some(1080),
                height: Some(1080),
            }],
            like_count: Some(42),
            comment_count: Some(5),
            is_video: false,
            video_view_count: None,
            timestamp: 1700000000,
        }
    }

    #[test]
    fn embed_contains_og_title_with_username() {
        let data = sample_image_data();
        let html = render_embed(&data, "cattgram.com", None);
        assert!(html.contains(r#"og:title" content="@testuser"#));
    }

    #[test]
    fn embed_contains_og_image_for_image_media() {
        let data = sample_image_data();
        let html = render_embed(&data, "cattgram.com", None);
        assert!(html.contains(r#"og:image" content="https://cdn.example.com/image.jpg"#));
        assert!(html.contains(r#"twitter:card" content="summary_large_image"#));
    }

    #[test]
    fn embed_contains_oembed_link() {
        let data = sample_image_data();
        let html = render_embed(&data, "cattgram.com", None);
        assert!(html.contains(r#"application/json+oembed"#));
        assert!(html.contains("cattgram.com/oembed"));
    }

    #[test]
    fn embed_escapes_html_in_caption() {
        let mut data = sample_image_data();
        data.caption = Some("<script>alert('xss')</script>".to_string());
        let html = render_embed(&data, "cattgram.com", None);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn embed_truncates_long_caption() {
        let mut data = sample_image_data();
        data.caption = Some("a".repeat(500));
        let html = render_embed(&data, "cattgram.com", None);
        // 300 chars + "..."
        assert!(html.contains(&format!("{}...", "a".repeat(300))));
    }

    #[test]
    fn embed_shows_video_tags() {
        let mut data = sample_image_data();
        data.is_video = true;
        data.video_view_count = Some(1000);
        data.media = vec![Media {
            media_type: MediaType::Video,
            url: "https://cdn.example.com/video.mp4".to_string(),
            thumbnail_url: Some("https://cdn.example.com/thumb.jpg".to_string()),
            width: Some(1920),
            height: Some(1080),
        }];
        let html = render_embed(&data, "cattgram.com", None);
        assert!(html.contains(r#"og:video" content="https://cdn.example.com/video.mp4"#));
        assert!(html.contains(r#"twitter:card" content="player"#));
        assert!(html.contains(r#"og:image" content="https://cdn.example.com/thumb.jpg"#));
        assert!(html.contains("1,000 views"));
    }

    #[test]
    fn embed_carousel_shows_slide_info() {
        let mut data = sample_image_data();
        data.media.push(Media {
            media_type: MediaType::Image,
            url: "https://cdn.example.com/image2.jpg".to_string(),
            thumbnail_url: None,
            width: Some(1080),
            height: Some(1080),
        });
        let html = render_embed(&data, "cattgram.com", Some(2));
        assert!(html.contains("Slide 2/2"));
        assert!(html.contains("image2.jpg"));
    }

    #[test]
    fn format_number_adds_commas() {
        assert_eq!(format_number(0), "0");
        assert_eq!(format_number(999), "999");
        assert_eq!(format_number(1000), "1,000");
        assert_eq!(format_number(1234567), "1,234,567");
    }
}
