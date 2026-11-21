use crate::dto::error::{ErrorAction, ErrorCode, ErrorDto, UrlTarget};
use hengst_core::auth::AuthError;
use hengst_core::loaders::{LoaderHint, LoaderKind};
use hengst_core::Error;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Maps a core error to what the web view gets. Exhaustive on purpose (no `_` arm): a new core
/// variant breaks the build until it is mapped.
pub fn error_dto(e: &Error) -> ErrorDto {
    use ErrorAction::{OpenSettings, OpenUrl, Retry, SignIn};
    let mut params = BTreeMap::new();
    let mut put = |key: &str, value: String| {
        params.insert(key.to_string(), value);
    };
    let (code, action) = match e {
        Error::Http { url, .. } => {
            put("url", strip_query(url));
            (ErrorCode::Network, Some(Retry))
        }
        Error::Status { status, .. } => {
            put("status", status.to_string());
            (ErrorCode::ServerError, Some(Retry))
        }
        Error::Offline { .. } => (ErrorCode::Offline, Some(Retry)),
        Error::Checksum { .. } => (ErrorCode::ChecksumMismatch, Some(Retry)),
        Error::UnsafePath(_) | Error::UnknownPlaceholder { .. } | Error::ParentMismatch { .. } => {
            (ErrorCode::UnsafeData, None)
        }
        Error::UnknownVersion(version) => {
            put("version", version.clone());
            (ErrorCode::UnknownVersion, None)
        }
        Error::NoRuntime { .. } => (ErrorCode::NoJavaRuntime, Some(OpenSettings)),
        Error::OfflineLaunchDisabled => (ErrorCode::OfflineAccountBlocked, None),
        Error::NoLoaderVersions { loader, game, hint } => {
            loader_params(&mut put, loader, game);
            if let Some(hint) = hint {
                put("hint", hint_key(*hint).to_string());
            }
            (ErrorCode::NoLoaderVersions, None)
        }
        Error::UnknownLoaderVersion {
            loader,
            game,
            version,
        } => {
            loader_params(&mut put, loader, game);
            put("version", version.clone());
            (ErrorCode::UnknownLoaderVersion, None)
        }
        Error::LoaderUnsupported {
            loader,
            game,
            reason,
        } => {
            loader_params(&mut put, loader, game);
            put("reason", reason.clone());
            (ErrorCode::LoaderUnsupported, None)
        }
        Error::TooLarge { .. } | Error::InvalidChecksumFile { .. } | Error::NoChecksum { .. } => {
            (ErrorCode::ServerError, None)
        }
        Error::Installer(_) | Error::Zip(_) => (ErrorCode::InstallerInvalid, None),
        Error::InstallFailed(_) => (ErrorCode::InstallFailed, Some(Retry)),
        // The log path stays in the core: the web view gets the step and the exit code only.
        Error::Processor { jar, code, .. } => {
            put("step", jar.clone());
            put("code", code.to_string());
            (ErrorCode::InstallStepFailed, None)
        }
        Error::NativesInUse { .. } => (ErrorCode::NativesInUse, None),
        Error::LockTimeout { .. } => (ErrorCode::LockTimeout, Some(Retry)),
        Error::Cancelled => (ErrorCode::Cancelled, None),
        Error::CredentialStore(_) => (ErrorCode::CredentialStore, None),
        Error::Io(_) => (ErrorCode::FileSystem, None),
        Error::Json(_) => (ErrorCode::Internal, None),
        Error::Auth(auth) => match auth {
            AuthError::MissingClientId => (ErrorCode::MissingClientId, None),
            AuthError::DeviceCodeExpired => (ErrorCode::DeviceCodeExpired, Some(SignIn)),
            AuthError::LoginDeclined => (ErrorCode::LoginDeclined, Some(SignIn)),
            AuthError::AppMisconfigured { .. } => (ErrorCode::AppMisconfigured, None),
            AuthError::LoginExpired { name } => {
                put("name", name.clone());
                (ErrorCode::LoginExpired, Some(SignIn))
            }
            AuthError::NoXboxAccount => (
                ErrorCode::NoXboxAccount,
                Some(OpenUrl {
                    target: UrlTarget::XboxSignup,
                }),
            ),
            AuthError::XboxRegionBlocked => (ErrorCode::XboxRegionBlocked, None),
            AuthError::XboxAdultVerification => (ErrorCode::XboxAdultVerification, None),
            AuthError::XboxChildAccount => (ErrorCode::XboxChildAccount, None),
            AuthError::XboxRefused { code } => {
                put("code", code.to_string());
                (ErrorCode::XboxRefused, None)
            }
            AuthError::AppNotApproved => (
                ErrorCode::AppNotApproved,
                Some(OpenUrl {
                    target: UrlTarget::SignInHelp,
                }),
            ),
            AuthError::GameNotOwned => (ErrorCode::GameNotOwned, None),
            AuthError::NoProfile => (
                ErrorCode::NoProfile,
                Some(OpenUrl {
                    target: UrlTarget::MinecraftProfile,
                }),
            ),
            AuthError::AccountNotFound { .. } => (ErrorCode::AccountNotFound, None),
            AuthError::NoAccount => (ErrorCode::NoAccount, Some(SignIn)),
            AuthError::BadResponse(_) => (ErrorCode::ServerError, Some(Retry)),
        },
    };
    ErrorDto {
        code,
        params,
        action,
        detail: redact(&e.to_string()),
    }
}

/// `loader` is the core's slug; the web view gets the display name (the slug itself when it is
/// not a known loader) and the slug separately.
fn loader_params(put: &mut impl FnMut(&str, String), slug: &str, game: &str) {
    let name: &str = match LoaderKind::from_str(slug) {
        Ok(kind) => kind.display_name(),
        Err(_) => slug,
    };
    put("loader", name.to_string());
    put("loaderSlug", slug.to_string());
    put("game", game.to_string());
}

fn hint_key(hint: LoaderHint) -> &'static str {
    match hint {
        LoaderHint::NeoForgeStartsAt1201 => "neoForgeStartsAt1201",
        LoaderHint::FabricStartsAt114 => "fabricStartsAt114",
        LoaderHint::LegacyFabricServesOld => "legacyFabricServesOld",
    }
}

/// Makes a core message safe for the "Details" disclosure: URL query parts, bearer tokens and
/// the value of an `Authorization` header are removed. Local paths stay.
pub fn redact(text: &str) -> String {
    let text = strip_query(text);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while !rest.is_empty() {
        let lower = rest.to_ascii_lowercase();
        let bearer = lower.find("bearer ");
        let header = ["authorization:", "authorization="]
            .iter()
            .filter_map(|k| lower.find(k).map(|i| (i, k.len())))
            .min();
        // The earlier of the two patterns wins. Both are ASCII and lowercasing ASCII keeps byte
        // offsets, so the offsets index `rest` as well.
        match (bearer, header) {
            (Some(b), h) if h.is_none_or(|(i, _)| b < i) => {
                let end = b + "bearer ".len();
                out.push_str(&rest[..end]);
                let value = rest[end..]
                    .find(|c: char| c.is_whitespace() || matches!(c, ';' | ',' | ')' | '"' | '\''))
                    .unwrap_or(rest.len() - end);
                // The word "bearer" in prose ("the bearer of ...") has no token after it.
                let word = &rest[end..end + value];
                let looks_like_token = word.len() >= 16
                    || word.chars().any(|c| {
                        c.is_ascii_digit() || matches!(c, '.' | '-' | '_' | '/' | '+' | '=')
                    });
                out.push_str(if looks_like_token { "***" } else { word });
                rest = &rest[end + value..];
            }
            (_, Some((i, len))) => {
                let end = i + len;
                out.push_str(&rest[..end]);
                out.push_str(" ***");
                let value = rest[end..]
                    .find(['\n', '\r', ';'])
                    .unwrap_or(rest.len() - end);
                rest = &rest[end + value..];
            }
            _ => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
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

#[cfg(test)]
mod mapping_tests {
    use super::*;
    use crate::dto::error::{ErrorAction, ErrorCode, UrlTarget};
    use hengst_core::auth::AuthError;
    use hengst_core::loaders::LoaderHint;
    use std::path::PathBuf;

    // The desktop crate does not depend on reqwest: the transport error is built by the core's
    // client and its type is only ever inferred.
    fn http(url: &str) -> Error {
        let source = hengst_core::download::http_client()
            .get("not a url")
            .build()
            .unwrap_err();
        Error::Http {
            url: url.into(),
            source,
        }
    }

    fn offline() -> Error {
        let source = hengst_core::download::http_client()
            .get("not a url")
            .build()
            .unwrap_err();
        Error::Offline {
            path: PathBuf::from("C:/h/x"),
            url: "https://x/y".into(),
            source,
        }
    }

    type Row = (
        Error,
        ErrorCode,
        Option<ErrorAction>,
        Vec<(&'static str, &'static str)>,
    );

    fn row(
        e: Error,
        code: ErrorCode,
        action: Option<ErrorAction>,
        params: &[(&'static str, &'static str)],
    ) -> Row {
        (e, code, action, params.to_vec())
    }

    fn auth(e: AuthError, code: ErrorCode, action: Option<ErrorAction>) -> Row {
        row(Error::Auth(e), code, action, &[])
    }

    /// One sample per variant: expected code, action and the exact set of param keys.
    fn table() -> Vec<Row> {
        use ErrorAction::{OpenSettings, OpenUrl, Retry, SignIn};
        let path = || PathBuf::from("C:/h/x");
        vec![
            row(
                http("https://api.example/v1?access_token=SECRET"),
                ErrorCode::Network,
                Some(Retry),
                &[("url", "https://api.example/v1")],
            ),
            row(
                Error::Status {
                    url: "https://x/y".into(),
                    status: 503,
                },
                ErrorCode::ServerError,
                Some(Retry),
                &[("status", "503")],
            ),
            row(offline(), ErrorCode::Offline, Some(Retry), &[]),
            row(
                Error::Checksum {
                    path: path(),
                    expected: "a".into(),
                    actual: "b".into(),
                },
                ErrorCode::ChecksumMismatch,
                Some(Retry),
                &[],
            ),
            row(
                Error::UnsafePath("../x".into()),
                ErrorCode::UnsafeData,
                None,
                &[],
            ),
            row(
                Error::UnknownPlaceholder {
                    version: "1.0".into(),
                    key: "k".into(),
                },
                ErrorCode::UnsafeData,
                None,
                &[],
            ),
            row(
                Error::ParentMismatch {
                    expected: "1.0".into(),
                    found: "2.0".into(),
                },
                ErrorCode::UnsafeData,
                None,
                &[],
            ),
            row(
                Error::UnknownVersion("9.9".into()),
                ErrorCode::UnknownVersion,
                None,
                &[("version", "9.9")],
            ),
            row(
                Error::NoRuntime {
                    component: "c".into(),
                    platform: "p".into(),
                },
                ErrorCode::NoJavaRuntime,
                Some(OpenSettings),
                &[],
            ),
            row(
                Error::OfflineLaunchDisabled,
                ErrorCode::OfflineAccountBlocked,
                None,
                &[],
            ),
            row(
                Error::NoLoaderVersions {
                    loader: "legacy-fabric".into(),
                    game: "1.21".into(),
                    hint: Some(LoaderHint::FabricStartsAt114),
                },
                ErrorCode::NoLoaderVersions,
                None,
                &[
                    ("game", "1.21"),
                    ("hint", "fabricStartsAt114"),
                    ("loader", "Legacy Fabric"),
                    ("loaderSlug", "legacy-fabric"),
                ],
            ),
            row(
                Error::NoLoaderVersions {
                    loader: "fabric".into(),
                    game: "1.21".into(),
                    hint: None,
                },
                ErrorCode::NoLoaderVersions,
                None,
                &[
                    ("game", "1.21"),
                    ("loader", "Fabric"),
                    ("loaderSlug", "fabric"),
                ],
            ),
            row(
                Error::NoLoaderVersions {
                    loader: "mystery".into(),
                    game: "1.21".into(),
                    hint: Some(LoaderHint::NeoForgeStartsAt1201),
                },
                ErrorCode::NoLoaderVersions,
                None,
                &[
                    ("game", "1.21"),
                    ("hint", "neoForgeStartsAt1201"),
                    ("loader", "mystery"),
                    ("loaderSlug", "mystery"),
                ],
            ),
            row(
                Error::NoLoaderVersions {
                    loader: "legacy-fabric".into(),
                    game: "1.21".into(),
                    hint: Some(LoaderHint::LegacyFabricServesOld),
                },
                ErrorCode::NoLoaderVersions,
                None,
                &[
                    ("game", "1.21"),
                    ("hint", "legacyFabricServesOld"),
                    ("loader", "Legacy Fabric"),
                    ("loaderSlug", "legacy-fabric"),
                ],
            ),
            row(
                Error::UnknownLoaderVersion {
                    loader: "neoforge".into(),
                    game: "1.21".into(),
                    version: "9".into(),
                },
                ErrorCode::UnknownLoaderVersion,
                None,
                &[
                    ("game", "1.21"),
                    ("loader", "NeoForge"),
                    ("loaderSlug", "neoforge"),
                    ("version", "9"),
                ],
            ),
            row(
                Error::LoaderUnsupported {
                    loader: "forge".into(),
                    game: "1.5".into(),
                    reason: "too old".into(),
                },
                ErrorCode::LoaderUnsupported,
                None,
                &[
                    ("game", "1.5"),
                    ("loader", "Forge"),
                    ("loaderSlug", "forge"),
                    ("reason", "too old"),
                ],
            ),
            row(
                Error::TooLarge {
                    url: "https://x".into(),
                    limit: 1,
                },
                ErrorCode::ServerError,
                None,
                &[],
            ),
            row(
                Error::InvalidChecksumFile {
                    url: "https://x".into(),
                },
                ErrorCode::ServerError,
                None,
                &[],
            ),
            row(
                Error::NoChecksum {
                    url: "https://x".into(),
                },
                ErrorCode::ServerError,
                None,
                &[],
            ),
            row(
                Error::Installer("bad".into()),
                ErrorCode::InstallerInvalid,
                None,
                &[],
            ),
            row(
                Error::Zip(zip::result::ZipError::FileNotFound),
                ErrorCode::InstallerInvalid,
                None,
                &[],
            ),
            row(
                Error::InstallFailed("x".into()),
                ErrorCode::InstallFailed,
                Some(Retry),
                &[],
            ),
            row(
                Error::Processor {
                    jar: "binarypatcher".into(),
                    code: 1,
                    log: PathBuf::from("C:/h/logs/x.log"),
                },
                ErrorCode::InstallStepFailed,
                None,
                &[("code", "1"), ("step", "binarypatcher")],
            ),
            row(
                Error::NativesInUse { dir: path() },
                ErrorCode::NativesInUse,
                None,
                &[],
            ),
            row(
                Error::LockTimeout {
                    label: "forge".into(),
                    waited_secs: 600,
                    lock: path(),
                },
                ErrorCode::LockTimeout,
                Some(Retry),
                &[],
            ),
            row(Error::Cancelled, ErrorCode::Cancelled, None, &[]),
            row(
                Error::CredentialStore("denied".into()),
                ErrorCode::CredentialStore,
                None,
                &[],
            ),
            row(
                Error::Io(std::io::Error::other("disk gone")),
                ErrorCode::FileSystem,
                None,
                &[],
            ),
            row(
                Error::Json(serde_json::from_str::<u8>("x").unwrap_err()),
                ErrorCode::Internal,
                None,
                &[],
            ),
            auth(AuthError::MissingClientId, ErrorCode::MissingClientId, None),
            auth(
                AuthError::DeviceCodeExpired,
                ErrorCode::DeviceCodeExpired,
                Some(SignIn),
            ),
            auth(
                AuthError::LoginDeclined,
                ErrorCode::LoginDeclined,
                Some(SignIn),
            ),
            auth(
                AuthError::AppMisconfigured {
                    code: "AADSTS1".into(),
                },
                ErrorCode::AppMisconfigured,
                None,
            ),
            row(
                Error::Auth(AuthError::LoginExpired {
                    name: "Mira_K".into(),
                }),
                ErrorCode::LoginExpired,
                Some(SignIn),
                &[("name", "Mira_K")],
            ),
            auth(
                AuthError::NoXboxAccount,
                ErrorCode::NoXboxAccount,
                Some(OpenUrl {
                    target: UrlTarget::XboxSignup,
                }),
            ),
            auth(
                AuthError::XboxRegionBlocked,
                ErrorCode::XboxRegionBlocked,
                None,
            ),
            auth(
                AuthError::XboxAdultVerification,
                ErrorCode::XboxAdultVerification,
                None,
            ),
            auth(
                AuthError::XboxChildAccount,
                ErrorCode::XboxChildAccount,
                None,
            ),
            row(
                Error::Auth(AuthError::XboxRefused { code: 2148916238 }),
                ErrorCode::XboxRefused,
                None,
                &[("code", "2148916238")],
            ),
            auth(
                AuthError::AppNotApproved,
                ErrorCode::AppNotApproved,
                Some(OpenUrl {
                    target: UrlTarget::SignInHelp,
                }),
            ),
            auth(AuthError::GameNotOwned, ErrorCode::GameNotOwned, None),
            auth(
                AuthError::NoProfile,
                ErrorCode::NoProfile,
                Some(OpenUrl {
                    target: UrlTarget::MinecraftProfile,
                }),
            ),
            auth(
                AuthError::AccountNotFound {
                    selector: "x".into(),
                },
                ErrorCode::AccountNotFound,
                None,
            ),
            auth(AuthError::NoAccount, ErrorCode::NoAccount, Some(SignIn)),
            auth(
                AuthError::BadResponse("xsts"),
                ErrorCode::ServerError,
                Some(Retry),
            ),
        ]
    }

    #[test]
    fn every_error_maps_to_a_code() {
        for (error, code, action, params) in table() {
            let dto = error_dto(&error);
            assert_eq!(dto.code, code, "{error:?}");
            assert_eq!(dto.action, action, "{error:?}");
            let got: Vec<(&str, &str)> = dto
                .params
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            assert_eq!(got, params, "params of {error:?}");
        }
    }

    #[test]
    fn processor_error_carries_no_path_to_the_web_view() {
        let dto = error_dto(&Error::Processor {
            jar: "binarypatcher".into(),
            code: 1,
            log: PathBuf::from("C:/h/logs/x.log"),
        });
        assert_eq!(dto.code, ErrorCode::InstallStepFailed);
        assert!(dto.action.is_none());
        assert!(!serde_json::to_string(&dto.params)
            .unwrap()
            .contains("x.log"));
    }

    #[test]
    fn error_detail_never_contains_secret() {
        for (error, ..) in table() {
            let json = serde_json::to_string(&error_dto(&error)).unwrap();
            for banned in [
                "Secret(",
                "<redacted>",
                "accessToken",
                "SECRET",
                "access_token",
            ] {
                assert!(!json.contains(banned), "{banned} in {json}");
            }
        }
    }

    #[test]
    fn detail_drops_query_strings_bearer_tokens_and_authorization_headers() {
        let msg = "GET https://x/a?code=ABC&state=Q failed; Authorization: Bearer eyJ.abc.def; retry with bearer tok-123 or authorization=Basic Zm9v\nnext line";
        let out = redact(msg);
        assert!(!out.contains("ABC") && !out.contains("state=Q"), "{out}");
        assert!(
            !out.contains("eyJ") && !out.contains("tok-123") && !out.contains("Zm9v"),
            "{out}"
        );
        assert!(out.contains("https://x/a failed"), "{out}");
        assert!(out.ends_with("next line"), "{out}");
        // Ordinary words stay.
        assert_eq!(redact("the bearer of bad news"), "the bearer of bad news");
        assert_eq!(redact("authorization failed"), "authorization failed");
    }

    #[test]
    fn a_core_message_carrying_a_token_reaches_the_web_view_clean() {
        let e = Error::Status {
            url: "https://api.example/login?access_token=SECRET-TOKEN".into(),
            status: 401,
        };
        let json = serde_json::to_string(&error_dto(&e)).unwrap();
        assert!(!json.contains("SECRET-TOKEN"), "{json}");
        let e = Error::InstallFailed("sent Authorization: Bearer SECRET-TOKEN".into());
        let json = serde_json::to_string(&error_dto(&e)).unwrap();
        assert!(!json.contains("SECRET-TOKEN"), "{json}");
    }

    #[test]
    fn serialized_shape_is_stable() {
        let json = serde_json::to_string(&error_dto(&Error::Cancelled)).unwrap();
        assert_eq!(
            json,
            r#"{"code":"cancelled","params":{},"action":null,"detail":"the operation was cancelled"}"#
        );
        let action = serde_json::to_string(&ErrorAction::OpenUrl {
            target: UrlTarget::SignInHelp,
        })
        .unwrap();
        assert_eq!(action, r#"{"kind":"openUrl","target":"signInHelp"}"#);
        assert_eq!(
            serde_json::to_string(&ErrorAction::Retry).unwrap(),
            r#"{"kind":"retry"}"#
        );
    }
}
