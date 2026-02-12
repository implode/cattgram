/// Renders the static homepage HTML.
pub fn render_home() -> String {
    r#"<!DOCTYPE html>
<html lang="en" data-theme="light">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Cattgram</title>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css">
</head>
<body>
<main class="container">
<hgroup>
<h1>Cattgram</h1>
<p>Fix Instagram embeds for Discord and Telegram</p>
</hgroup>

<section>
<h2>Usage</h2>
<p>Replace <code>instagram.com</code> with <code>cattgram.com</code> (or whatever domain you deploy to) in any Instagram link.</p>
<p><strong>Example:</strong></p>
<pre><code>https://cattgram.com/p/ABC123/</code></pre>
</section>

<section>
<h2>Supported URL Formats</h2>
<ul>
<li><code>/p/:postID</code> &mdash; Posts</li>
<li><code>/reel/:postID</code> &mdash; Reels</li>
<li><code>/reels/:postID</code> &mdash; Reels (alternate)</li>
<li><code>/tv/:postID</code> &mdash; IGTV</li>
<li><code>/stories/:username/:storyID</code> &mdash; Stories</li>
</ul>
</section>

<section>
<h2>Query Parameters</h2>
<ul>
<li><code>?direct=true</code> &mdash; Redirect directly to the media file (image or video URL)</li>
<li><code>?img_index=N</code> &mdash; Select a specific slide in a carousel post (1-based index)</li>
</ul>
</section>

<footer>
<p><small>Powered by Cloudflare Workers</small></p>
</footer>
</main>
</body>
</html>"#
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_contains_title() {
        let html = render_home();
        assert!(html.contains("<title>Cattgram</title>"));
    }

    #[test]
    fn home_contains_pico_css() {
        let html = render_home();
        assert!(html.contains("picocss/pico@2"));
    }

    #[test]
    fn home_contains_supported_formats() {
        let html = render_home();
        assert!(html.contains("/p/:postID"));
        assert!(html.contains("/reel/:postID"));
        assert!(html.contains("/stories/:username/:storyID"));
    }

    #[test]
    fn home_contains_query_params() {
        let html = render_home();
        assert!(html.contains("?direct=true"));
        assert!(html.contains("?img_index=N"));
    }
}
