use url::Url;
use worker::*;

pub async fn handle(req: Request, _ctx: RouteContext<()>) -> Result<Response> {
    let req_url = req.url().map_err(|e| Error::RustError(e.to_string()))?;

    let text = get_query_param(&req_url, "text").unwrap_or_default();
    let url = get_query_param(&req_url, "url").unwrap_or_default();

    let json = serde_json::json!({
        "author_name": text,
        "author_url": url,
        "provider_name": "Cattgram",
        "provider_url": "https://cattgram.com",
        "title": "Instagram",
        "type": "link",
        "version": "1.0"
    });

    let body = serde_json::to_string(&json)
        .map_err(|e| Error::RustError(format!("JSON serialization error: {e}")))?;

    let headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    Ok(Response::ok(body)?.with_headers(headers))
}

/// Extracts a single query parameter value from a URL.
fn get_query_param(url: &Url, key: &str) -> Option<String> {
    url.query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}
