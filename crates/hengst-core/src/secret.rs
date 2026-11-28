use serde::Deserialize;
use std::fmt;

/// Placeholder printed instead of a secret value.
pub(crate) const REDACTED: &str = "<redacted>";

/// A confidential string (for example an access token). Never printed by
/// `Debug`, has no `Display` and cannot be serialized.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Secret {
        Secret(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Secret({REDACTED})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_the_value() {
        let s = Secret::new("eyJhbGciOi.secret");
        assert_eq!(format!("{s:?}"), "Secret(<redacted>)");
        assert_eq!(s.expose(), "eyJhbGciOi.secret");
    }
}
