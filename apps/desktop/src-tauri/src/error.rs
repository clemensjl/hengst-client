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

/// Removes URL query parts (`?a=b` up to the next space, quote or closing bracket) from a
/// message, so logs and error details never carry them.
pub fn strip_query(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for c in text.chars() {
        if skipping {
            if c.is_whitespace() || matches!(c, ')' | ']' | '"' | '\'' | '>') {
                skipping = false;
                out.push(c);
            }
        } else if c == '?' {
            skipping = true;
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_parts_are_removed_up_to_the_next_delimiter() {
        assert_eq!(
            strip_query(
                "request to http://x/a?token=1&b=2 failed: error for url (http://x/a?token=1&b=2)"
            ),
            "request to http://x/a failed: error for url (http://x/a)"
        );
        assert_eq!(strip_query("no query here"), "no query here");
    }
}
