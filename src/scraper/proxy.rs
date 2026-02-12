use worker::*;

/// Makes a fetch request through a residential proxy if configured.
///
/// Expects these env secrets:
/// - PROXY_HOST: proxy hostname (e.g. "brd.superproxy.io")
/// - PROXY_PORT: proxy port (e.g. "22225")
/// - PROXY_USERNAME: proxy username
/// - PROXY_PASSWORD: proxy password
///
/// Since CF Workers can't use HTTP CONNECT proxies for HTTPS targets,
/// this uses Bright Data's REST API at api.brightdata.com/request
/// with the zone name extracted from the proxy username.
///
/// If secrets are not set, falls back to direct fetch.
pub async fn proxy_fetch(
    target_url: &str,
    method: Method,
    headers: Headers,
    body: Option<String>,
    env: &Env,
) -> Result<worker::Response> {
    let username = env.secret("PROXY_USERNAME").ok().map(|s| s.to_string());
    let password = env.secret("PROXY_PASSWORD").ok().map(|s| s.to_string());

    match (username, password) {
        (Some(user), Some(pass)) => {
            residential_proxy_fetch(target_url, method, headers, body, &user, &pass).await
        }
        _ => {
            console_log!("[proxy] no proxy config, fetching directly");
            direct_fetch(target_url, method, headers, body).await
        }
    }
}

/// Fetch via residential proxy using Bright Data's REST API.
///
/// Extracts the zone name from the proxy username (format: brd-customer-XXX-zone-ZONE_NAME)
/// and uses it with the REST API at api.brightdata.com/request.
async fn residential_proxy_fetch(
    target_url: &str,
    method: Method,
    original_headers: Headers,
    body: Option<String>,
    username: &str,
    password: &str,
) -> Result<worker::Response> {
    console_log!("[proxy] routing through residential proxy: {}", target_url);

    // Extract zone name from username (brd-customer-XXX-zone-ZONE_NAME or just use as-is)
    let zone = extract_zone(username).unwrap_or_else(|| "residential".to_string());
    console_log!("[proxy] using zone: {}", zone);

    let method_str = match method {
        Method::Get => "GET",
        Method::Post => "POST",
        _ => "GET",
    };

    // Collect original headers into the proxy payload
    let mut proxy_headers = serde_json::Map::new();
    let forward_keys = [
        "User-Agent", "Accept", "Accept-Language", "Cookie",
        "Content-Type", "Origin", "Referer",
        "X-Ig-App-Id", "X-Fb-Lsd", "X-Asbd-Id", "X-Fb-Friendly-Name",
        "X-Requested-With",
        "Sec-Fetch-Dest", "Sec-Fetch-Mode", "Sec-Fetch-Site",
        "Sec-Ch-Ua", "Sec-Ch-Ua-Mobile", "Sec-Ch-Ua-Platform",
    ];
    for key in &forward_keys {
        if let Ok(Some(val)) = original_headers.get(key) {
            proxy_headers.insert(key.to_string(), serde_json::Value::String(val));
        }
    }

    let mut payload = serde_json::json!({
        "zone": zone,
        "url": target_url,
        "format": "raw",
        "method": method_str,
        "country": "us",
    });

    if !proxy_headers.is_empty() {
        payload["headers"] = serde_json::Value::Object(proxy_headers);
    }

    if let Some(ref b) = body {
        payload["body"] = serde_json::Value::String(b.clone());
    }

    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| Error::RustError(format!("JSON serialize error: {e}")))?;

    console_log!("[proxy] payload: {}", &payload_str[..payload_str.len().min(300)]);

    // REST API at api.brightdata.com/request always uses Bearer token
    let auth_header = format!("Bearer {}", password);
    console_log!("[proxy] auth: Bearer {}...", &password[..password.len().min(10)]);

    let headers = Headers::new();
    headers.set("Authorization", &auth_header)?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(payload_str.into()));

    let request = Request::new_with_init("https://api.brightdata.com/request", &init)?;
    let resp = Fetch::Request(request).send().await?;

    console_log!("[proxy] response status={}", resp.status_code());
    Ok(resp)
}

/// Extract zone name from Bright Data proxy username.
/// Format: "brd-customer-XXXXX-zone-ZONE_NAME" or "brd-customer-XXXXX-zone-ZONE_NAME-..."
fn extract_zone(username: &str) -> Option<String> {
    let zone_idx = username.find("-zone-")?;
    let after_zone = &username[zone_idx + 6..];
    // Zone name ends at next '-' or end of string
    let zone = match after_zone.find('-') {
        Some(end) => &after_zone[..end],
        None => after_zone,
    };
    if zone.is_empty() {
        None
    } else {
        Some(zone.to_string())
    }
}

/// Simple base64 encoding for Basic auth.
fn base64_encode(input: &[u8]) -> String {
    const CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Direct fetch without proxy.
async fn direct_fetch(
    target_url: &str,
    method: Method,
    headers: Headers,
    body: Option<String>,
) -> Result<worker::Response> {
    let mut init = RequestInit::new();
    init.with_method(method).with_headers(headers);
    if let Some(b) = body {
        init.with_body(Some(b.into()));
    }

    let request = Request::new_with_init(target_url, &init)?;
    Fetch::Request(request).send().await
}
