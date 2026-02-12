use worker::*;

use super::embed_page::parse_shortcode_media;
use super::proxy::proxy_fetch;
use super::types::InstaData;

const CHROME_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
                          (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";
const IG_APP_ID: &str = "936619743392459";

pub async fn fetch_graphql(post_id: &str, doc_id: &str, env: &Env) -> Result<Option<InstaData>> {
    let variables = format!(
        r#"{{"shortcode":"{}","fetch_comment_count":40,"parent_comment_count":24,"child_comment_count":3,"fetch_like_count":10,"fetch_tagged_user_count":null,"fetch_preview_comment_count":2,"has_threaded_comments":true,"hoisted_comment_id":null,"hoisted_reply_id":null}}"#,
        post_id
    );

    let body = build_graphql_body(&variables, doc_id);
    let target_url = "https://www.instagram.com/api/graphql";

    // Try direct fetch first (usually returns null from datacenter IPs)
    console_log!("[graphql] trying direct fetch for {} with doc_id={}", post_id, doc_id);
    let result = match direct_graphql_fetch(target_url, &body).await {
        Ok(mut r) => {
            let status = r.status_code();
            let text = r.text().await?;
            console_log!("[graphql] direct status={} len={} first_200={}", status, text.len(), &text[..text.len().min(200)]);
            parse_graphql_response(&text, post_id)
        }
        Err(e) => {
            console_log!("[graphql] direct fetch error: {:?}", e);
            None
        }
    };

    if result.is_some() {
        return Ok(result);
    }

    // Fall back to residential proxy
    console_log!("[graphql] trying via proxy");
    let headers = build_graphql_headers()?;
    let mut resp = proxy_fetch(target_url, Method::Post, headers, Some(body), env).await?;
    let status = resp.status_code();
    let text = resp.text().await?;
    console_log!("[graphql] proxy status={} len={} first_200={}", status, text.len(), &text[..text.len().min(200)]);

    Ok(parse_graphql_response(&text, post_id))
}

/// Builds the form-encoded POST body with all the obfuscation parameters
/// that Instagram expects from a real browser session.
fn build_graphql_body(variables: &str, doc_id: &str) -> String {
    form_urlencode(&[
        ("av", "0"),
        ("__d", "www"),
        ("__user", "0"),
        ("__a", "1"),
        ("__req", "k"),
        ("__hs", "19888.HYP:instagram_web_pkg.2.1..0.0"),
        ("dpr", "2"),
        ("__ccg", "UNKNOWN"),
        ("__rev", "1014227545"),
        ("__s", "trbjos:n8dn55:yev1rm"),
        ("__hsi", "7380500578385702299"),
        ("__dyn", "7xeUjG1mxu1syUbFp40NonwgU7SbzEdF8aUco2qwJw5ux609vCwjE1xoswaq0yE6ucw5Mx62G5UswoEcE7O2l0Fwqo31w9a9wtUd8-U2zxe2GewGw9a362W2K0zK5o4q3y1Sx-0iS2Sq2-azo7u3C2u2J0bS1LwTwKG1pg2fwxyo6O1FwlEcUed6goK2O4UrAwCAxW6Uf9EObzVU8U"),
        ("__csr", "n2Yfg_5hcQAG5mPtfEzil8Wn-DpKGBXhdczlAhrK8uHBAGuKCJeCieLDyExenh68aQAKta8p8ShogKkF5yaUBqCpF9XHmmhoBXyBKbQp0HCwDjqoOepV8Tzk8xeXqAGFTVoCciGaCgvGUtVU-u5Vp801nrEkO0rC58xw41g0VW07ISyie2W1v7F0CwYwwwvEkw8K5cM0VC1dwdi0hCbc094w6MU1xE02lzw"),
        ("__comet_req", "7"),
        ("lsd", "AVoPBTXMX0Y"),
        ("jazoest", "2882"),
        ("__spin_r", "1014227545"),
        ("__spin_b", "trunk"),
        ("__spin_t", "1718406700"),
        ("fb_api_caller_class", "RelayModern"),
        ("fb_api_req_friendly_name", "PolarisPostActionLoadPostQueryQuery"),
        ("variables", variables),
        ("server_timestamps", "true"),
        ("doc_id", doc_id),
    ])
}

/// Parses a GraphQL JSON response into InstaData.
fn parse_graphql_response(text: &str, post_id: &str) -> Option<InstaData> {
    if text.contains("require_login") || text.contains("not-logged-in") {
        console_log!("[graphql] response requires login");
        return None;
    }

    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            console_log!("[graphql] JSON parse error: {}", e);
            return None;
        }
    };

    if let Some(obj) = json.as_object() {
        console_log!("[graphql] top-level keys: {:?}", obj.keys().collect::<Vec<_>>());
    }

    let media_obj = json.get("data").and_then(|d| {
        console_log!("[graphql] data keys: {:?}", d.as_object().map(|o| o.keys().collect::<Vec<_>>()));
        d.get("xdt_shortcode_media")
            .or_else(|| d.get("shortcode_media"))
    })?;

    // xdt_shortcode_media can be JSON null when IP-blocked
    if media_obj.is_null() {
        console_log!("[graphql] media object is null (likely IP-blocked)");
        return None;
    }

    parse_shortcode_media(media_obj, post_id)
}

/// Builds the full set of browser-spoofing headers for GraphQL requests.
fn build_graphql_headers() -> Result<Headers> {
    let headers = Headers::new();
    headers.set("Accept", "*/*")?;
    headers.set("Accept-Language", "en-US,en;q=0.9")?;
    headers.set("Content-Type", "application/x-www-form-urlencoded")?;
    headers.set("Origin", "https://www.instagram.com")?;
    headers.set("Referer", "https://www.instagram.com/")?;
    headers.set("Priority", "u=1, i")?;
    headers.set("Sec-Ch-Prefers-Color-Scheme", "dark")?;
    headers.set("Sec-Ch-Ua", r#""Google Chrome";v="125", "Chromium";v="125", "Not.A/Brand";v="24""#)?;
    headers.set("Sec-Ch-Ua-Full-Version-List", r#""Google Chrome";v="125.0.6422.142", "Chromium";v="125.0.6422.142", "Not.A/Brand";v="24.0.0.0""#)?;
    headers.set("Sec-Ch-Ua-Mobile", "?0")?;
    headers.set("Sec-Ch-Ua-Model", r#""""#)?;
    headers.set("Sec-Ch-Ua-Platform", r#""macOS""#)?;
    headers.set("Sec-Ch-Ua-Platform-Version", r#""12.7.4""#)?;
    headers.set("Sec-Fetch-Dest", "empty")?;
    headers.set("Sec-Fetch-Mode", "cors")?;
    headers.set("Sec-Fetch-Site", "same-origin")?;
    headers.set("User-Agent", CHROME_UA)?;
    headers.set("X-Asbd-Id", "129477")?;
    headers.set("X-Fb-Lsd", "AVoPBTXMX0Y")?;
    headers.set("X-Fb-Friendly-Name", "PolarisPostActionLoadPostQueryQuery")?;
    headers.set("X-Ig-App-Id", IG_APP_ID)?;
    Ok(headers)
}

/// Makes a direct GraphQL POST request from the CF Worker without any proxy.
async fn direct_graphql_fetch(url: &str, body: &str) -> Result<worker::Response> {
    let headers = build_graphql_headers()?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(body.to_string().into()));

    let request = Request::new_with_init(url, &init)?;
    Fetch::Request(request).send().await
}

/// Simple form URL encoding for key-value pairs.
fn form_urlencode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                url::form_urlencoded::byte_serialize(k.as_bytes()).collect::<String>(),
                url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>(),
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}
