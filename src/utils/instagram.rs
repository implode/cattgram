use url::Url;

const INSTAGRAM_BASE64: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Allowed query parameters on Instagram CDN URLs.
const ALLOWED_CDN_PARAMS: [&str; 8] = [
    "stp",
    "dst",
    "_nc_cat",
    "_nc_ohc",
    "ccb",
    "oh",
    "oe",
    "_nc_sid",
];

/// Converts a numeric Instagram media ID to a shortcode.
///
/// Uses Instagram's custom base64 alphabet, dividing repeatedly by 64
/// and mapping each remainder to the corresponding character.
pub fn mediaid_to_code(media_id: u64) -> String {
    if media_id == 0 {
        return String::from("A");
    }

    let mut id = media_id;
    let mut chars = Vec::new();
    while id > 0 {
        let remainder = (id % 64) as usize;
        chars.push(INSTAGRAM_BASE64[remainder] as char);
        id /= 64;
    }
    chars.reverse();
    chars.into_iter().collect()
}

/// Converts a shortcode back to a numeric media ID.
///
/// Reverses the `mediaid_to_code` process using Instagram's base64 alphabet.
pub fn code_to_mediaid(code: &str) -> Option<u64> {
    let mut id: u64 = 0;
    for ch in code.chars() {
        let pos = INSTAGRAM_BASE64.iter().position(|&c| c == ch as u8)?;
        id = id.checked_mul(64)?.checked_add(pos as u64)?;
    }
    Some(id)
}

/// Strips tracking parameters from an Instagram CDN URL.
///
/// Retains only the allowlisted query parameters (`stp`, `dst`, `_nc_cat`,
/// `_nc_ohc`, `ccb`, `oh`, `oe`, `_nc_sid`). Returns the original URL
/// unchanged if parsing fails.
pub fn normalize_cdn_url(url_str: &str) -> String {
    let Ok(mut parsed) = Url::parse(url_str) else {
        return url_str.to_string();
    };

    let kept_params: Vec<(String, String)> = parsed
        .query_pairs()
        .filter(|(key, _)| ALLOWED_CDN_PARAMS.contains(&key.as_ref()))
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    if kept_params.is_empty() {
        parsed.set_query(None);
    } else {
        parsed.query_pairs_mut().clear().extend_pairs(&kept_params);
    }

    parsed.to_string()
}

/// Extracts the post ID (shortcode) from an Instagram URL path.
///
/// Handles paths like `/p/ABC123/`, `/reel/ABC123/`, `/tv/ABC123/`,
/// with or without trailing slashes and extra path segments.
pub fn extract_post_id(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for (i, segment) in segments.iter().enumerate() {
        if matches!(*segment, "p" | "reel" | "tv" | "reels") {
            return segments.get(i + 1).map(|s| s.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- mediaid_to_code ---

    #[test]
    fn mediaid_converts_known_values() {
        assert_eq!(mediaid_to_code(2481276043892498677), "CJvQ2ph5iD1");
    }

    #[test]
    fn mediaid_zero_returns_a() {
        assert_eq!(mediaid_to_code(0), "A");
    }

    #[test]
    fn mediaid_small_value() {
        assert_eq!(mediaid_to_code(1), "B");
        assert_eq!(mediaid_to_code(63), "_");
        assert_eq!(mediaid_to_code(64), "BA");
    }

    // --- normalize_cdn_url ---

    #[test]
    fn cdn_url_keeps_allowed_params() {
        let input = "https://scontent.cdninstagram.com/v/image.jpg?stp=dst-jpg&_nc_cat=1&ccb=1-7&oh=abc&oe=def&tracking=bad&efg=remove";
        let result = normalize_cdn_url(input);

        assert!(result.contains("stp=dst-jpg"));
        assert!(result.contains("_nc_cat=1"));
        assert!(result.contains("ccb=1-7"));
        assert!(result.contains("oh=abc"));
        assert!(result.contains("oe=def"));
        assert!(!result.contains("tracking"));
        assert!(!result.contains("efg=remove"));
    }

    #[test]
    fn cdn_url_strips_all_unknown_params() {
        let input = "https://cdn.example.com/image.jpg?tracking=1&fbclid=abc";
        let result = normalize_cdn_url(input);
        assert_eq!(result, "https://cdn.example.com/image.jpg");
    }

    #[test]
    fn cdn_url_returns_original_on_parse_failure() {
        let input = "not-a-url";
        assert_eq!(normalize_cdn_url(input), "not-a-url");
    }

    #[test]
    fn cdn_url_no_query_string() {
        let input = "https://cdn.example.com/image.jpg";
        assert_eq!(normalize_cdn_url(input), "https://cdn.example.com/image.jpg");
    }

    // --- extract_post_id ---

    #[test]
    fn extracts_from_p_path() {
        assert_eq!(extract_post_id("/p/ABC123/"), Some("ABC123".to_string()));
    }

    #[test]
    fn extracts_from_reel_path() {
        assert_eq!(
            extract_post_id("/reel/DEF456/"),
            Some("DEF456".to_string())
        );
    }

    #[test]
    fn extracts_from_tv_path() {
        assert_eq!(extract_post_id("/tv/GHI789/"), Some("GHI789".to_string()));
    }

    #[test]
    fn extracts_without_trailing_slash() {
        assert_eq!(extract_post_id("/p/XYZ"), Some("XYZ".to_string()));
    }

    #[test]
    fn extracts_with_extra_segments() {
        assert_eq!(
            extract_post_id("/p/ABC123/embed/captioned"),
            Some("ABC123".to_string())
        );
    }

    #[test]
    fn returns_none_for_unrecognized_path() {
        assert_eq!(extract_post_id("/explore/tags/cat/"), None);
    }

    #[test]
    fn returns_none_for_empty_path() {
        assert_eq!(extract_post_id("/"), None);
        assert_eq!(extract_post_id(""), None);
    }

    #[test]
    fn returns_none_for_prefix_without_id() {
        assert_eq!(extract_post_id("/p/"), None);
    }
}
