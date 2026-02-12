/// Escapes a string for safe embedding in HTML.
///
/// Replaces `&`, `<`, `>`, `"`, and `'` with their HTML entity equivalents.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes a string for safe embedding inside a JSON string value.
///
/// Handles backslashes, double quotes, newlines, carriage returns, tabs,
/// and other control characters (U+0000..U+001F).
pub fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                // Unicode escape for remaining control chars
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escapes_all_special_chars() {
        assert_eq!(
            escape_html(r#"<script>alert("x&y")</script>"#),
            "&lt;script&gt;alert(&quot;x&amp;y&quot;)&lt;/script&gt;"
        );
    }

    #[test]
    fn html_escapes_single_quote() {
        assert_eq!(escape_html("it's"), "it&#x27;s");
    }

    #[test]
    fn html_passthrough_plain_text() {
        assert_eq!(escape_html("hello world"), "hello world");
    }

    #[test]
    fn json_escapes_backslash_and_quote() {
        assert_eq!(escape_json_string(r#"a\"b"#), r#"a\\\"b"#);
    }

    #[test]
    fn json_escapes_newlines_and_tabs() {
        assert_eq!(escape_json_string("line1\nline2\ttab"), "line1\\nline2\\ttab");
    }

    #[test]
    fn json_escapes_carriage_return() {
        assert_eq!(escape_json_string("a\rb"), "a\\rb");
    }

    #[test]
    fn json_escapes_control_chars() {
        assert_eq!(escape_json_string("\x00\x1f"), "\\u0000\\u001f");
    }

    #[test]
    fn json_passthrough_plain_text() {
        assert_eq!(escape_json_string("hello world"), "hello world");
    }

    #[test]
    fn empty_strings() {
        assert_eq!(escape_html(""), "");
        assert_eq!(escape_json_string(""), "");
    }
}
