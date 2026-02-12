const BOT_SIGNATURES: [&str; 31] = [
    "bot",
    "facebook",
    "embed",
    "got",
    "firefox/92",
    "firefox/38",
    "curl",
    "wget",
    "go-http",
    "yahoo",
    "generator",
    "whatsapp",
    "preview",
    "link",
    "proxy",
    "vkshare",
    "images",
    "analyzer",
    "index",
    "crawl",
    "spider",
    "python",
    "cfnetwork",
    "node",
    "mastodon",
    "http.rb",
    "discord",
    "telegram",
    "slack",
    "redditbot",
    "dataprovider",
];

/// Returns `true` if the user-agent string matches any known bot signature.
pub fn is_bot(user_agent: &str) -> bool {
    let ua_lower = user_agent.to_ascii_lowercase();
    BOT_SIGNATURES.iter().any(|sig| ua_lower.contains(sig))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_bots() {
        assert!(is_bot("Twitterbot/1.0"));
        assert!(is_bot("facebookexternalhit/1.1"));
        assert!(is_bot("Mozilla/5.0 (compatible; Discordbot/2.0)"));
        assert!(is_bot("TelegramBot (like TwitterBot)"));
        assert!(is_bot("Slackbot-LinkExpanding 1.0"));
        assert!(is_bot("WhatsApp/2.23"));
        assert!(is_bot("python-requests/2.28.0"));
        assert!(is_bot("curl/7.88.1"));
        assert!(is_bot("wget/1.21"));
        assert!(is_bot("Go-http-client/1.1"));
        assert!(is_bot("redditbot/1.0"));
    }

    #[test]
    fn detects_case_insensitive() {
        assert!(is_bot("DISCORDBOT"));
        assert!(is_bot("WhatsApp"));
        assert!(is_bot("CURL/8.0"));
    }

    #[test]
    fn ignores_real_browsers() {
        assert!(!is_bot(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0.0.0 Safari/537.36"
        ));
        assert!(!is_bot(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 Safari/604.1"
        ));
        assert!(!is_bot(
            "Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0"
        ));
    }

    #[test]
    fn empty_ua_is_not_bot() {
        assert!(!is_bot(""));
    }
}
