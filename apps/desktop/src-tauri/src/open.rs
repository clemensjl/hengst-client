//! Opening pages in the system browser. The web view names a `UrlTarget`; the address is fixed
//! here, so no URL from JavaScript ever reaches the opener.

use crate::dto::error::{ErrorCode, ErrorDto, UrlTarget};
use crate::error::redact;
use std::collections::BTreeMap;
use std::fmt::Display;

pub fn resolve_url(target: &UrlTarget) -> &'static str {
    match target {
        UrlTarget::Repository => "https://github.com/clemensjl/hengst-client",
        UrlTarget::SignInHelp => "https://github.com/clemensjl/hengst-client#readme",
        UrlTarget::XboxSignup => "https://www.xbox.com/",
        UrlTarget::MinecraftProfile => "https://www.minecraft.net/msaprofile/mygames/editprofile",
        UrlTarget::DeviceLogin => "https://www.microsoft.com/link",
        // The "Build" section of the README, which says how to set the Microsoft client id.
        UrlTarget::BuildHelp => "https://github.com/clemensjl/hengst-client#build",
    }
}

/// Opens the fixed address of `target` through `opener`; a refusal becomes `OpenFailed`.
pub fn open_target<E: Display>(
    target: &UrlTarget,
    opener: impl FnOnce(&'static str) -> Result<(), E>,
) -> Result<(), ErrorDto> {
    opener(resolve_url(target)).map_err(|e| ErrorDto {
        code: ErrorCode::OpenFailed,
        params: BTreeMap::new(),
        action: None,
        detail: redact(&e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::error::{ErrorCode, UrlTarget};

    const ALL: [UrlTarget; 6] = [
        UrlTarget::Repository,
        UrlTarget::SignInHelp,
        UrlTarget::XboxSignup,
        UrlTarget::MinecraftProfile,
        UrlTarget::DeviceLogin,
        UrlTarget::BuildHelp,
    ];

    #[test]
    fn url_targets_are_fixed_https_urls() {
        for t in ALL {
            assert!(resolve_url(&t).starts_with("https://"), "{t:?}");
            assert!(!resolve_url(&t).contains('?'), "{t:?}");
        }
        assert_eq!(
            resolve_url(&UrlTarget::DeviceLogin),
            "https://www.microsoft.com/link"
        );
    }

    #[test]
    fn every_target_resolves_to_its_address() {
        let want = [
            (
                UrlTarget::Repository,
                "https://github.com/clemensjl/hengst-client",
            ),
            (
                UrlTarget::SignInHelp,
                "https://github.com/clemensjl/hengst-client#readme",
            ),
            (UrlTarget::XboxSignup, "https://www.xbox.com/"),
            (
                UrlTarget::MinecraftProfile,
                "https://www.minecraft.net/msaprofile/mygames/editprofile",
            ),
            (UrlTarget::DeviceLogin, "https://www.microsoft.com/link"),
            (
                UrlTarget::BuildHelp,
                "https://github.com/clemensjl/hengst-client#build",
            ),
        ];
        assert_eq!(want.len(), ALL.len());
        for (target, url) in want {
            assert_eq!(resolve_url(&target), url, "{target:?}");
        }
    }

    #[test]
    fn opening_passes_exactly_the_resolved_url_to_the_opener() {
        for t in ALL {
            let mut seen = vec![];
            open_target(&t, |url| {
                seen.push(url.to_string());
                Ok::<(), String>(())
            })
            .unwrap();
            assert_eq!(seen, [resolve_url(&t)], "{t:?}");
        }
    }

    #[test]
    fn a_failing_opener_is_a_typed_error_without_a_panic() {
        let err = open_target(&UrlTarget::BuildHelp, |_| {
            Err::<(), _>("no default browser: https://x/y?token=SECRET")
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OpenFailed);
        assert!(err.action.is_none());
        assert!(err.detail.contains("no default browser"));
        assert!(!err.detail.contains("SECRET"), "{}", err.detail);
    }

    #[test]
    fn only_a_known_target_name_deserializes() {
        for (name, target) in [
            ("signInHelp", UrlTarget::SignInHelp),
            ("buildHelp", UrlTarget::BuildHelp),
            ("deviceLogin", UrlTarget::DeviceLogin),
        ] {
            let parsed: UrlTarget = serde_json::from_str(&format!("\"{name}\"")).unwrap();
            assert_eq!(parsed, target);
        }
        for bad in [
            "\"https://evil.example/\"",
            "\"SignInHelp\"",
            "\"\"",
            "{\"url\":\"https://evil.example/\"}",
        ] {
            assert!(serde_json::from_str::<UrlTarget>(bad).is_err(), "{bad}");
        }
    }
}
