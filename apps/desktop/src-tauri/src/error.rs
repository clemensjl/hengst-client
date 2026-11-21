use crate::dto::error::{ErrorAction, ErrorCode, ErrorDto};
use hengst_core::Error;
use std::collections::BTreeMap;

/// Maps a core error to what the web view gets. Task 4 makes this exhaustive.
pub fn error_dto(e: &Error) -> ErrorDto {
    let (code, action) = match e {
        Error::Http { .. } | Error::Offline { .. } => {
            (ErrorCode::Network, Some(ErrorAction::Retry))
        }
        _ => (ErrorCode::Internal, None),
    };
    ErrorDto {
        code,
        params: BTreeMap::new(),
        action,
        detail: strip_query(&e.to_string()),
    }
}

/// Removes the query part of every http(s) URL in a message, so logs and error details never carry
/// tokens or other parameters. A `?` outside a URL (ordinary prose) stays.
pub fn strip_query(text: &str) -> String {
    let is_end = |c: char| c.is_whitespace() || matches!(c, ')' | ']' | '"' | '\'' | '>');
    let mut out = String::with_capacity(text.len());
    let mut in_url = false;
    let mut skipping = false;
    for (i, c) in text.char_indices() {
        if !in_url && (text[i..].starts_with("http://") || text[i..].starts_with("https://")) {
            in_url = true;
        }
        if in_url && is_end(c) {
            in_url = false;
            skipping = false;
        }
        if in_url && c == '?' {
            skipping = true;
        }
        if !skipping {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parts_of_urls_are_removed_up_to_the_next_delimiter() {
        assert_eq!(
            strip_query(
                "request to http://x/a?token=1&b=2 failed: error for url (https://x/a?token=1&b=2)"
            ),
            "request to http://x/a failed: error for url (https://x/a)"
        );
        assert_eq!(
            strip_query("GET https://api.example/v1/login?access_token=abc.def-ghi done"),
            "GET https://api.example/v1/login done"
        );
    }

    #[test]
    fn question_marks_in_prose_stay() {
        let sentence = "Is the server reachable? Check the proxy; what now?";
        assert_eq!(strip_query(sentence), sentence);
        assert_eq!(
            strip_query("Is it http://x/a?t=1 or not? Unclear."),
            "Is it http://x/a or not? Unclear."
        );
    }
}
