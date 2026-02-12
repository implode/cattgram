# Cattgram

A fast, serverless Instagram embed fixer for Discord, Telegram, and other social platforms. Detects bot user-agents and returns richly formatted HTML with OpenGraph and Twitter Card meta tags so embeds render properly. Non-bot traffic is redirected to the original Instagram URL.

Built in Rust, compiled to WebAssembly, and deployed on Cloudflare Workers.

## Features

- **31+ Bot Detections**: Discord, Telegram, Slack, WhatsApp, Mastodon, Reddit, and more
- **Multi-Strategy Instagram Scraping**: Fallback chain ensures reliable data extraction
  - KV Cache (24-hour TTL)
  - Instagram Embed Page (JSON + HTML parsing)
  - GraphQL API (with direct and proxy fallback)
  - Instagram Private API (PAPI) with session support
  - Thumbnail fallback for when all else fails
- **Residential Proxy Integration**: Bright Data REST API for bypassing Instagram's datacenter IP blocks
- **Complete Media Support**: Posts, reels, stories, carousels, videos, and images
- **Direct Media Redirects**: Fast endpoints to get direct image/video URLs
- **oEmbed Endpoint**: Standard oEmbed JSON responses
- **OpenGraph + Twitter Cards**: Proper rich preview formatting for embeds
- **Session Cookie Support**: Optional Instagram session for PAPI access
- **Smart URL Normalization**: Strips tracking parameters from Instagram CDN URLs

## Tech Stack

| Component | Technology | Version |
|-----------|-----------|---------|
| Language | Rust | 2021 edition |
| Runtime | Cloudflare Workers | WASM |
| HTTP Client | worker crate | 0.7 |
| Serialization | serde + serde_json | 1.0 |
| Caching | Cloudflare KV | 24h TTL |
| Proxy | Bright Data REST API | - |

## Quick Start

### Prerequisites

- Rust 1.70+ with `wasm32-unknown-unknown` target
- Node.js 18+ (for wrangler)
- Cloudflare account with Workers enabled
- Bright Data residential proxy credentials (optional but recommended)

### Installation

```bash
cd cattgram
cargo fetch
```

### Configuration

Create a `wrangler.toml` file (or update the existing one):

```toml
name = "cattgram"
main = "build/worker/shim.mjs"
compatibility_date = "2024-11-01"

[build]
command = "cargo install -q worker-build && worker-build --release"

[vars]
GRAPHQL_DOC_ID = "8845758582119845"

[[kv_namespaces]]
binding = "CACHE"
id = "YOUR_KV_NAMESPACE_ID"
```

#### Required Secrets

Set these via `wrangler secret put`:

```bash
wrangler secret put PROXY_USERNAME    # Bright Data proxy username
wrangler secret put PROXY_PASSWORD    # Bright Data API token
wrangler secret put IG_COOKIE         # (Optional) Instagram sessionid cookie
```

**Proxy Username Format**: `brd-customer-{CUSTOMER_ID}-zone-{ZONE_NAME}`

**IG_COOKIE Format**: Either:
- Raw sessionid value: `{USER_ID}:{TOKEN}:{VERSION}:{HASH}`
- Full cookie: `sessionid={USER_ID}:{TOKEN}:{VERSION}:{HASH}`
- The worker auto-detects and formats correctly

#### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| GRAPHQL_DOC_ID | Instagram GraphQL document ID for queries | `8845758582119845` |

### Build

```bash
cargo install -q worker-build
worker-build --release
```

This compiles Rust to WASM and generates the Worker entry point at `build/worker/shim.mjs`.

### Local Development

```bash
npx wrangler dev
```

Visit `http://localhost:8787` in your browser.

### Deploy

```bash
npx wrangler deploy
```

## Project Structure

```
cattgram/
├── src/
│   ├── lib.rs                 # Worker fetch event handler and router
│   ├── handlers/              # HTTP endpoint handlers
│   │   ├── mod.rs
│   │   ├── embed.rs           # POST embed endpoint (/p/:postID, /reel/:postID, etc)
│   │   ├── home.rs            # GET / landing page
│   │   ├── media.rs           # /images/:postID/:mediaNum, /videos/:postID/:mediaNum
│   │   └── oembed.rs          # /oembed oEmbed JSON endpoint
│   ├── scraper/               # Instagram data extraction logic
│   │   ├── mod.rs             # Orchestrator: cache -> embed -> graphql -> papi -> thumbnail
│   │   ├── types.rs           # InstaData and Media structs
│   │   ├── cache.rs           # Cloudflare KV cache (24h TTL)
│   │   ├── embed_page.rs      # Instagram embed page parser (JSON + HTML fallback)
│   │   ├── graphql.rs         # GraphQL API client with direct + proxy fallback
│   │   ├── papi.rs            # Instagram Private API (mobile app API)
│   │   └── proxy.rs           # Bright Data residential proxy integration
│   ├── templates/             # HTML generation
│   │   ├── mod.rs
│   │   ├── embed_html.rs      # Rich embed HTML with OG/Twitter Card tags
│   │   └── home_html.rs       # Landing page
│   └── utils/                 # Helper functions
│       ├── bot_detect.rs      # 31+ bot user-agent detection
│       ├── escape.rs          # HTML entity escaping
│       └── instagram.rs       # Shortcode <-> media ID conversion, URL parsing
├── Cargo.toml                 # Rust dependencies
├── wrangler.toml              # Cloudflare Workers config
├── .cargo/config.toml         # Rust build configuration
└── README.md
```

## API Routes

### GET /
Home page with documentation and status.

**Response**: HTML landing page

---

### GET /p/:postID
### GET /reel/:postID
### GET /reels/:postID
### GET /tv/:postID
### GET /stories/:username/:storyID

Instagram post embed endpoint. Detects bot user-agents and returns rich HTML with OpenGraph/Twitter Card meta tags.

**Query Parameters**:
- `img_index` (number, 1-based): Select specific carousel image
- `direct` (true/false): If true, redirect directly to media URL instead of returning HTML

**Bot Detection**: Returns HTML only to known bots (Discord, Telegram, Slack, etc). Regular browsers redirect to `https://www.instagram.com/p/:postID/`

**Example Response** (to Discord bot):
```html
<!DOCTYPE html>
<html>
<head>
  <meta property="og:title" content="@username">
  <meta property="og:description" content="Post caption...">
  <meta property="og:image" content="https://scontent.cdninstagram.com/...">
  <meta property="og:url" content="https://www.instagram.com/p/ABC123/">
  <meta name="twitter:card" content="summary_large_image">
</head>
<body><!-- Minimal body --></body>
</html>
```

**Error Handling**: If post data cannot be fetched, redirects to Instagram.

---

### GET /images/:postID/:mediaNum
Direct image redirect for carousel items.

**Path Parameters**:
- `postID` (string): Instagram post shortcode
- `mediaNum` (number, 1-based): Media item index

**Response**: 302 Redirect to image URL or Instagram post (if not found)

**Example**: `/images/ABC123/2` -> redirects to the 2nd image in a carousel

---

### GET /videos/:postID/:mediaNum
Direct video redirect for carousel items.

**Path Parameters**:
- `postID` (string): Instagram post shortcode
- `mediaNum` (number, 1-based): Media item index

**Response**: 302 Redirect to video URL or Instagram post (if not found)

**Example**: `/videos/ABC123/1` -> redirects to the 1st video in a carousel

---

### GET /oembed
oEmbed JSON endpoint for rich embed support.

**Query Parameters**:
- `text` (string, optional): Author name
- `url` (string, optional): Original URL

**Response**: JSON oEmbed object
```json
{
  "author_name": "text param value",
  "author_url": "url param value",
  "provider_name": "Cattgram",
  "provider_url": "https://cattgram.com",
  "title": "Instagram",
  "type": "link",
  "version": "1.0"
}
```

---

## Data Scraping Strategy

The scraper uses a **fallback chain** to maximize success rates despite Instagram's anti-scraping measures.

### 1. Cache Check (Cloudflare KV)
First request for a post checks the KV cache for existing data with a 24-hour TTL. Cache misses proceed to live scraping.

### 2. Embed Page Parser
Fetches `https://www.instagram.com/p/{postID}/embed/captioned/` which includes JSON metadata.

**Extraction Methods** (in order):
1. **JSON Blob**: Extracts `shortcode_media` JSON object from page HTML
   - Complete data including video URLs
   - Used for direct media redirects
2. **Context JSON**: Fallback to double-encoded `contextJSON` field
   - Same `shortcode_media` structure
   - Handles some edge cases
3. **HTML Fallback**: Scrapes `EmbeddedMediaImage` class and captions
   - Thumbnail URLs only, no video URLs
   - Triggers GraphQL retry for better data

**Proxy Integration**: Uses Bright Data residential proxy if configured. Supports Instagram session cookie for better success rates.

### 3. GraphQL API
Queries Instagram's internal GraphQL endpoint at `https://www.instagram.com/api/graphql` with proper browser spoofing headers.

**Features**:
- Direct fetch attempt (fails from datacenter IPs)
- Automatic fallback to Bright Data proxy
- Browser headers to bypass rate limiting
- Handles login walls gracefully

**Doc ID**: Configurable via `GRAPHQL_DOC_ID` environment variable. Fallback to `25531498899829322` if not set.

### 4. Instagram Private API (PAPI)
Uses the Instagram mobile app API at `https://i.instagram.com/api/v1/media/{media_id}/info/`.

**Requirements**:
- Valid Instagram session cookie (`IG_COOKIE` secret)
- User ID automatically extracted from sessionid

**Features**:
- Direct fetch with session
- Proxy fallback
- Mobile app user-agent spoofing
- Carousel support

### 5. Thumbnail Fallback
If all scraping methods fail, returns the thumbnail extracted from the embed page (if available). Used only as a last resort.

### Success Indicators

Each method returns `InstaData` with:
- Post ID and username
- Caption text (optional)
- Media list (images/videos with URLs and dimensions)
- Engagement metrics (like count, comment count, view count)
- Timestamp

The orchestrator detects which method succeeded based on data richness:
- Complete JSON extraction? Use immediately.
- HTML-only thumbnail? Try GraphQL for richer data.
- Any complete data? Cache it.

## Bot Detection

The `is_bot()` function checks for 31+ known bot signatures in the User-Agent header (case-insensitive):

```
bot, facebook, embed, got, firefox/92, firefox/38, curl, wget,
go-http, yahoo, generator, whatsapp, preview, link, proxy,
vkshare, images, analyzer, index, crawl, spider, python,
cfnetwork, node, mastodon, http.rb, discord, telegram, slack,
redditbot, dataprovider
```

Non-matching user-agents (browsers, curl without bot signature, etc) are redirected to Instagram rather than served embeds.

## Proxy Configuration

Cattgram uses Bright Data's **REST API** (not HTTP CONNECT proxy) because Cloudflare Workers cannot establish CONNECT tunnels for HTTPS.

### REST API Flow

```
CF Worker
   |
   v
api.brightdata.com/request (POST)
   |
   v
Bright Data Residential Proxy
   |
   v
Instagram
```

### Setup

1. Get Bright Data credentials:
   - Customer ID
   - Zone name
   - API token (password)

2. Format the username: `brd-customer-{CUSTOMER_ID}-zone-{ZONE_NAME}`

3. Set secrets:
   ```bash
   wrangler secret put PROXY_USERNAME "brd-customer-123456-zone-residential"
   wrangler secret put PROXY_PASSWORD "your-api-token"
   ```

4. Zone name is extracted automatically and passed to the API.

### Fallback Behavior

If proxy secrets are not configured, all requests fall back to direct fetches. This works for:
- Embed page (usually okay)
- GraphQL (often blocked without residential IP)
- PAPI (depends on rate limits)

For best results, always configure the proxy.

## Instagram Session Cookie (Optional)

The Instagram Private API requires a valid session cookie. Get one by:

1. Log into Instagram in a browser
2. Open DevTools (F12)
3. Go to Application > Cookies > instagram.com
4. Find the `sessionid` cookie value
5. Set it as a secret:
   ```bash
   wrangler secret put IG_COOKIE "user_id:token:version:hash"
   ```

The worker auto-detects the format and adds the `ds_user_id` cookie for you.

Without `IG_COOKIE`, PAPI requests are skipped.

## Caching

### Cache Key Format
```
post:{postID}
```

Example: `post:CJvQ2ph5iD1`

### TTL
24 hours (86400 seconds)

### When Cache Is Used
1. **Check**: Every request checks the cache first
2. **Store**: Successful scrapes from any method are cached
3. **Bypass**: Direct redirects (/images/, /videos/) still fetch fresh data

### Cache Invalidation
Manual via Cloudflare dashboard or `wrangler kv:key delete` command. Automatic expiry after 24 hours.

## Media Type Handling

### Images
- Direct Instagram CDN URLs
- Dimensions included from JSON
- Tracking parameters stripped

### Videos
- Full video MP4 URLs from JSON or PAPI
- Thumbnail URLs for preview
- Video view count tracked

### Carousels
- Multiple media items in sequence
- Individual URLs for each item
- `/images/:postID/:mediaNum` routes for direct access

### Stories
- Similar structure to posts
- Numeric story ID converted to shortcode
- Support for `/stories/:username/:storyID` route

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Post not found (4xx) | Redirect to Instagram |
| Network error | Fall back to next scraping method |
| Cache deserialize error | Log and proceed to scraping |
| Invalid shortcode | Redirect to Instagram |
| Non-bot user-agent | Redirect to Instagram immediately |
| Missing media in carousel | Return 400 or redirect |

## Performance

### Latency
- Cache hit: ~50ms
- Embed page: ~200-500ms (including proxy)
- GraphQL: ~300-800ms (includes direct + fallback)
- PAPI: ~200-400ms
- Total p95: <2s for a cold request

### WASM Bundle Size
Optimized for Cloudflare's 1MB limit:
- Release build with LTO: ~350KB
- gzip: ~80KB

### Cache Efficiency
With diverse traffic patterns, expect 40-60% cache hit rates on popular posts.

## Development

### Running Tests
```bash
cargo test
```

Includes tests for:
- Bot detection (common bots and browsers)
- URL parsing and media ID conversion
- CDN URL normalization
- Post ID extraction

### Adding New Bot Signatures
Edit `src/utils/bot_detect.rs` and add to the `BOT_SIGNATURES` array:
```rust
const BOT_SIGNATURES: [&str; N] = [
    // ... existing signatures
    "mynewbot",
];
```

Then update the count in the array annotation and add a test case.

### Debugging
Enable console logs via Cloudflare dashboard:
```
wrangler tail
```

Log format: `[module] message` (e.g., `[scraper] cache HIT for ABC123`)

## Known Limitations

1. **Instagram Rate Limiting**: May block requests even with residential proxy during heavy load
2. **Login Walls**: Some private accounts or restricted content cannot be scraped
3. **Story Expiration**: Stories older than 24 hours are automatically deleted by Instagram
4. **Video URLs**: Embedded page doesn't always include full video URLs; requires GraphQL or PAPI
5. **Carousel Limits**: Very large carousels (100+ items) may be truncated by Instagram API
6. **Session Expiry**: Instagram session cookies expire periodically; manual refresh required

## Alternatives & Similar Projects

- **InstaFix** (original): Python implementation, deprecated
- **Disgramifier**: Similar concept, different approach
- **ProxyCat**: General-purpose proxy for social embeds

## License

MIT License. See LICENSE file for details.

## Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Submit a pull request

## Support

For issues, feature requests, or questions:
- Open a GitHub issue
- Check existing documentation
- Review the CLAUDE.md file for project context

## Security Notes

- Never commit secrets or API tokens
- Rotate Instagram session cookies regularly
- Monitor Bright Data usage to prevent unexpected charges
- Keep Rust dependencies updated via `cargo update`
- Review Cloudflare Workers environment for data handling compliance

## Deployment Checklist

Before going to production:

- [ ] Configure KV namespace in wrangler.toml
- [ ] Set PROXY_USERNAME and PROXY_PASSWORD secrets
- [ ] (Optional) Set IG_COOKIE secret for PAPI
- [ ] Test all routes locally with `wrangler dev`
- [ ] Verify bot detection with curl: `curl -A "Discordbot/1.0" https://your-domain/p/ABC123`
- [ ] Check logs via `wrangler tail`
- [ ] Monitor KV usage in Cloudflare dashboard
- [ ] Set up error tracking (e.g., Sentry integration)
- [ ] Document custom domain and rate limits
