use worker::*;

use super::types::InstaData;

const TTL_SECONDS: u64 = 86400; // 24 hours

fn cache_key(post_id: &str) -> String {
    format!("post:{post_id}")
}

pub async fn get_cached(post_id: &str, env: &Env) -> Result<Option<InstaData>> {
    let kv = env.kv("CACHE")?;
    let key = cache_key(post_id);

    match kv.get(&key).text().await? {
        Some(json) => {
            let data: InstaData = serde_json::from_str(&json)
                .map_err(|e| Error::RustError(format!("cache deserialize error: {e}")))?;
            Ok(Some(data))
        }
        None => Ok(None),
    }
}

pub async fn set_cached(post_id: &str, data: &InstaData, env: &Env) -> Result<()> {
    let kv = env.kv("CACHE")?;
    let key = cache_key(post_id);
    let json = serde_json::to_string(data)
        .map_err(|e| Error::RustError(format!("cache serialize error: {e}")))?;

    kv.put(&key, json)?
        .expiration_ttl(TTL_SECONDS)
        .execute()
        .await?;

    Ok(())
}
