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
        Error::Http { .. } => (ErrorCode::Network, Some(Retry)),
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
        Error::InstanceNotFound { id } => {
            put("id", id.clone());
            (ErrorCode::InstanceNotFound, Some(Retry))
        }
        Error::InstanceUnreadable { id, .. } => {
            put("id", id.clone());
            (ErrorCode::InstanceUnreadable, None)
        }
        Error::InstanceLinked { id } => {
            put("id", id.clone());
            (ErrorCode::InstanceLinked, None)
        }
        Error::InstancePartlyMoved { id } => {
            put("id", id.clone());
            (ErrorCode::RecycleBinPartial, None)
        }
        Error::InstanceMoveUnknown { id } => {
            put("id", id.clone());
            (ErrorCode::RecycleBinUnknown, None)
        }
        Error::InstanceRunning { id } => {
            put("id", id.clone());
            (ErrorCode::InstanceRunning, None)
        }
        Error::InstanceExists { id } => {
            put("id", id.clone());
            (ErrorCode::InstanceExists, None)
        }
        Error::InstanceFormatTooNew { id, format } => {
            put("id", id.clone());
            put("format", format.to_string());
            (ErrorCode::InstanceFormatTooNew, None)
        }
        Error::InvalidInstanceName(_) => (ErrorCode::InvalidInstanceName, None),
        Error::Cancelled => (ErrorCode::Cancelled, None),
        Error::CredentialStore(_) => (ErrorCode::CredentialStore, None),
        Error::Io(_) => (ErrorCode::FileSystem, None),
        Error::Json(_) => (ErrorCode::Internal, None),
        Error::Auth(auth) => match auth {
            AuthError::MissingClientId => (ErrorCode::MissingClientId, None),
            AuthError::DeviceCodeExpired => (ErrorCode::DeviceCodeExpired, Some(SignIn)),
            AuthError::LoginDeclined => (ErrorCode::LoginDeclined, Some(SignIn)),
            AuthError::BrowserLoginTimedOut => (ErrorCode::BrowserLoginTimedOut, Some(SignIn)),
            AuthError::LoopbackUnavailable => (ErrorCode::LoopbackUnavailable, None),
            AuthError::BrowserLoginFailed { code } => {
                put("code", code.clone());
                (ErrorCode::BrowserLoginFailed, Some(SignIn))
            }
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
        detail: match e {
            Error::Json(json) => json_detail(json),
            _ => strip_paths(&redact(&e.to_string())),
        },
    }
}

/// The error for an instance id the web view sent. An id that is not even a folder name (it fails
/// the id grammar, `UnsafePath`) names no instance: it is "not found", not "the downloaded data
/// was rejected", and "refresh the list" would not help, so it carries no action.
pub fn instance_id_error(e: &Error, id: &str) -> ErrorDto {
    match e {
        Error::UnsafePath(_) => ErrorDto {
            action: None,
            ..error_dto(&Error::InstanceNotFound { id: id.to_string() })
        },
        other => error_dto(other),
    }
}

/// serde quotes the offending value in its messages (a token in a response, a line of the accounts
/// file), so a JSON error is reduced to its category and position.
fn json_detail(e: &serde_json::Error) -> String {
    use serde_json::error::Category;
    let category = match e.classify() {
        Category::Io => "read error",
        Category::Syntax => "syntax error",
        Category::Data => "unexpected data",
        Category::Eof => "unexpected end",
    };
    format!(
        "invalid JSON data ({category}) at line {} column {}",
        e.line(),
        e.column()
    )
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
/// the value of an `Authorization` header are removed. Local paths are not touched here: callers
/// whose text may carry one run `strip_paths` on the result (`error_dto` does). That order matters:
/// a path runs to the next `: `, so it can swallow the scheme of a URL that follows it, after which
/// the query would no longer be found.
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

/// Replaces every absolute Windows path in `text` (`C:\Users\name\...`, a UNC path
/// `\\server\share\...`, a verbatim path `\\?\...`, also with doubled backslashes as a `Debug`
/// text has them) by `...\<last component>`, so a detail shown to the user or copied into a bug
/// report carries neither the Windows user name nor a server or share name. A path ends at `: `, a
/// line break, the end of the text, or the quote that opened it (one not followed by a letter or
/// digit, so `O'Brien` stays inside). Known residual case: in a path quoted with `'`, a name whose
/// apostrophe is followed by something else (`'C:\Users\O' Brien\x'`) ends the path at that
/// apostrophe, so the rest of the name stays in the text.
pub fn strip_paths(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let after_word = i > 0 && chars[i - 1].is_ascii_alphanumeric();
        let drive = chars[i].is_ascii_alphabetic()
            && chars.get(i + 1) == Some(&':')
            && matches!(chars.get(i + 2), Some('\\') | Some('/'));
        // Two or more backslashes, then a name: UNC (`\\server`) and verbatim (`\\?\`) paths.
        let backslashes = chars[i..].iter().take_while(|c| **c == '\\').count();
        let unc = backslashes >= 2
            && chars
                .get(i + backslashes)
                .is_some_and(|c| !c.is_whitespace() && !matches!(c, '"' | '\''));
        let starts_path = (drive || unc) && !after_word;
        if !starts_path {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // A quote ends the path only if the path was opened by that same quote, and only where it
        // is not followed by a letter or digit: a user name may contain an apostrophe (O'Brien).
        let opener = i
            .checked_sub(1)
            .map(|p| chars[p])
            .filter(|c| matches!(c, '"' | '\''));
        let mut end = i;
        while end < chars.len() {
            let c = chars[end];
            let colon_space = c == ':' && end > i + 1 && chars.get(end + 1) == Some(&' ');
            let closing =
                Some(c) == opener && !chars.get(end + 1).is_some_and(|n| n.is_alphanumeric());
            if colon_space || closing || matches!(c, '\n' | '\r') {
                break;
            }
            end += 1;
        }
        let path: String = chars[i..end].iter().collect();
        let last = path
            .rsplit(['\\', '/'])
            .find(|part| !part.is_empty())
            .unwrap_or("");
        out.push_str("...\\");
        out.push_str(last);
        i = end;
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
                &[],
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
                AuthError::BrowserLoginTimedOut,
                ErrorCode::BrowserLoginTimedOut,
                Some(SignIn),
            ),
            auth(
                AuthError::LoopbackUnavailable,
                ErrorCode::LoopbackUnavailable,
                None,
            ),
            row(
                Error::Auth(AuthError::BrowserLoginFailed {
                    code: "server_error".into(),
                }),
                ErrorCode::BrowserLoginFailed,
                Some(SignIn),
                &[("code", "server_error")],
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
            row(
                Error::InstanceNotFound { id: "9.9".into() },
                ErrorCode::InstanceNotFound,
                Some(Retry),
                &[("id", "9.9")],
            ),
            row(
                Error::InstanceUnreadable {
                    id: "9.9".into(),
                    reason: "x".into(),
                },
                ErrorCode::InstanceUnreadable,
                None,
                &[("id", "9.9")],
            ),
            row(
                Error::InstanceLinked { id: "9.9".into() },
                ErrorCode::InstanceLinked,
                None,
                &[("id", "9.9")],
            ),
            row(
                Error::InstancePartlyMoved { id: "9.9".into() },
                ErrorCode::RecycleBinPartial,
                None,
                &[("id", "9.9")],
            ),
            row(
                Error::InstanceMoveUnknown { id: "9.9".into() },
                ErrorCode::RecycleBinUnknown,
                None,
                &[("id", "9.9")],
            ),
            row(
                Error::InstanceRunning { id: "9.9".into() },
                ErrorCode::InstanceRunning,
                None,
                &[("id", "9.9")],
            ),
            row(
                Error::InstanceExists { id: "9.9".into() },
                ErrorCode::InstanceExists,
                None,
                &[("id", "9.9")],
            ),
            row(
                Error::InstanceFormatTooNew {
                    id: "9.9".into(),
                    format: 2,
                },
                ErrorCode::InstanceFormatTooNew,
                None,
                &[("format", "2"), ("id", "9.9")],
            ),
            row(
                Error::InvalidInstanceName("the name is empty".into()),
                ErrorCode::InvalidInstanceName,
                None,
                &[],
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
    fn paths_in_a_detail_lose_their_user_folder() {
        let single = "windows error: C:\\Users\\someone\\AppData\\Local\\Temp\\hengst\\t2: the destination is in use";
        assert_eq!(
            strip_paths(single),
            "windows error: ...\\t2: the destination is in use"
        );
        // The `Debug` form of an error doubles every backslash and quotes the text.
        let doubled = "Os { code: 1, description: \"windows error: C:\\\\Users\\\\someone\\\\x\\\\t2: in use\" }";
        let cleaned = strip_paths(doubled);
        assert!(
            !cleaned.contains("someone") && !cleaned.contains("Users"),
            "{cleaned}"
        );
        assert!(cleaned.contains("...\\t2: in use"), "{cleaned}");
        // Text without a path, and a lone drive letter in prose, stay as they are.
        assert_eq!(strip_paths("nothing to see: a:b"), "nothing to see: a:b");
        assert_eq!(strip_paths("C:/Users/someone/x"), "...\\x");
        assert_eq!(
            strip_paths("two C:\\a\\b and D:\\c\\d"),
            // Paths may contain spaces, so a path runs to its terminator and the text after the
            // first drive path goes with it. Nothing of a path is ever left.
            "two ...\\d"
        );
    }

    #[test]
    fn unc_and_verbatim_paths_lose_everything_but_their_last_component() {
        for (text, want) in [
            (
                "open \\\\fileserver\\share\\someone\\t2: denied",
                "open ...\\t2: denied",
            ),
            (
                "open \\\\?\\C:\\Users\\someone\\t2: denied",
                "open ...\\t2: denied",
            ),
            (
                "open \\\\?\\UNC\\fileserver\\share\\someone\\t2: denied",
                "open ...\\t2: denied",
            ),
            (
                "open \\\\?\\Volume{0b1c}\\Users\\someone\\t2",
                "open ...\\t2",
            ),
            // The `Debug` form doubles every backslash.
            (
                "\"open \\\\\\\\fileserver\\\\share\\\\someone\\\\t2: denied\"",
                "\"open ...\\t2: denied\"",
            ),
        ] {
            assert_eq!(strip_paths(text), want, "{text}");
        }
        // A backslash pair inside a word is no path start.
        assert_eq!(strip_paths("a\\\\b"), "a\\\\b");
    }

    #[test]
    fn an_apostrophe_in_a_user_name_does_not_end_the_path() {
        for (text, want) in [
            (
                "could not read C:\\Users\\O'Brien\\AppData\\settings.json: access denied",
                "could not read ...\\settings.json: access denied",
            ),
            (
                "could not read 'C:\\Users\\O'Brien\\AppData\\settings.json': access denied",
                "could not read '...\\settings.json': access denied",
            ),
            (
                "could not read \"C:\\Users\\O'Brien\\AppData\\settings.json\": access denied",
                "could not read \"...\\settings.json\": access denied",
            ),
            ("open \\\\fileserver\\share\\O'Brien\\t2", "open ...\\t2"),
            // An apostrophe followed by a letter stays in the path, one followed by a space ends
            // a quoted one.
            (
                "could not read C:\\Users\\someone\\world's end: access denied",
                "could not read ...\\world's end: access denied",
            ),
            (
                "could not read 'C:\\Users\\someone\\world' and more",
                "could not read '...\\world' and more",
            ),
        ] {
            assert_eq!(strip_paths(text), want, "{text}");
        }
    }

    /// Every error whose text can carry a local path: the web view gets neither the user name nor
    /// the server or share name.
    #[test]
    fn path_carrying_errors_reach_the_web_view_without_the_path() {
        let source = || {
            hengst_core::download::http_client()
                .get("not a url")
                .build()
                .unwrap_err()
        };
        let paths = [
            "C:\\Users\\someone\\AppData\\Roaming\\HengstClient\\x",
            "\\\\fileserver\\share\\someone\\HengstClient\\x",
            "\\\\?\\C:\\Users\\someone\\HengstClient\\x",
            "\\\\?\\UNC\\fileserver\\share\\someone\\HengstClient\\x",
        ];
        for p in paths {
            let errors = [
                Error::Io(std::io::Error::other(format!(
                    "could not read {p}\\settings.json: access denied"
                ))),
                // The two texts that block saving the settings (`settings::load_settings`).
                Error::Io(std::io::Error::other(format!(
                    "could not move the damaged settings file aside, it stays as it is: rename {p}\\settings.json: in use"
                ))),
                Error::Offline {
                    path: PathBuf::from(p),
                    url: "https://x/y".into(),
                    source: source(),
                },
                Error::Checksum {
                    path: PathBuf::from(p),
                    expected: "a".into(),
                    actual: "b".into(),
                },
                Error::NativesInUse {
                    dir: PathBuf::from(p),
                },
                Error::LockTimeout {
                    label: "forge".into(),
                    waited_secs: 600,
                    lock: PathBuf::from(p),
                },
                Error::Processor {
                    jar: "binarypatcher".into(),
                    code: 1,
                    log: PathBuf::from(p),
                },
            ];
            for e in errors {
                let json = serde_json::to_string(&error_dto(&e)).unwrap();
                for banned in ["someone", "Users", "fileserver", "share", "HengstClient"] {
                    assert!(!json.contains(banned), "{banned} in {json}");
                }
            }
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
    fn json_errors_carry_category_and_position_only() {
        // serde quotes the offending value in its message; the detail must not.
        let quoted = serde_json::from_str::<u8>("\"SECRET-TOKEN-abc123\"").unwrap_err();
        assert!(
            quoted.to_string().contains("SECRET-TOKEN-abc123"),
            "the premise"
        );
        for input in [
            "\"SECRET-TOKEN-abc123\"",
            "{\"access_token\": \"SECRET-TOKEN-abc123\" ",
            "[1, 2, \"SECRET-TOKEN-abc123\"]",
            "{\"a\": SECRET-TOKEN-abc123}",
        ] {
            let source = serde_json::from_str::<u8>(input).unwrap_err();
            let dto = error_dto(&Error::Json(source));
            assert_eq!(dto.code, ErrorCode::Internal);
            let json = serde_json::to_string(&dto).unwrap();
            assert!(!json.contains("SECRET-TOKEN"), "{json}");
            assert!(!json.contains("abc123"), "{json}");
            assert!(dto.detail.contains("line 1 column"), "{}", dto.detail);
        }
        let dto = error_dto(&Error::Json(quoted));
        assert_eq!(
            dto.detail,
            "invalid JSON data (unexpected data) at line 1 column 21"
        );
    }

    #[tokio::test]
    async fn a_real_transport_error_with_a_query_url_reaches_the_web_view_clean() {
        // A closed local port: no network, a real reqwest error whose text names the URL.
        let url = "http://127.0.0.1:9/path?access_token=SECRET-TOKEN-xyz&state=Q";
        let fail = || async {
            hengst_core::download::http_client()
                .get(url)
                .send()
                .await
                .unwrap_err()
        };
        let source = fail().await;
        assert!(
            source.to_string().contains("SECRET-TOKEN-xyz"),
            "the premise: {source}"
        );
        let http = Error::Http {
            url: url.into(),
            source,
        };
        let offline = Error::Offline {
            path: PathBuf::from("C:/h/x"),
            url: url.into(),
            source: fail().await,
        };
        for error in [http, offline] {
            let json = serde_json::to_string(&error_dto(&error)).unwrap();
            assert!(!json.contains("SECRET-TOKEN-xyz"), "{json}");
            assert!(!json.contains("state=Q"), "{json}");
        }
    }

    /// The wire name of every code, written out once: renaming or dropping a code changes what the
    /// web view sees, so it must be a deliberate edit of this list. The match is exhaustive, so a
    /// new variant breaks the build until it is listed here.
    fn wire(code: ErrorCode) -> &'static str {
        match code {
            ErrorCode::Internal => "internal",
            ErrorCode::InvalidInput => "invalidInput",
            ErrorCode::Network => "network",
            ErrorCode::Offline => "offline",
            ErrorCode::ServerError => "serverError",
            ErrorCode::ChecksumMismatch => "checksumMismatch",
            ErrorCode::UnsafeData => "unsafeData",
            ErrorCode::UnknownVersion => "unknownVersion",
            ErrorCode::NoJavaRuntime => "noJavaRuntime",
            ErrorCode::OfflineAccountBlocked => "offlineAccountBlocked",
            ErrorCode::NoLoaderVersions => "noLoaderVersions",
            ErrorCode::UnknownLoaderVersion => "unknownLoaderVersion",
            ErrorCode::LoaderUnsupported => "loaderUnsupported",
            ErrorCode::InstallerInvalid => "installerInvalid",
            ErrorCode::InstallFailed => "installFailed",
            ErrorCode::InstallStepFailed => "installStepFailed",
            ErrorCode::NativesInUse => "nativesInUse",
            ErrorCode::LockTimeout => "lockTimeout",
            ErrorCode::CredentialStore => "credentialStore",
            ErrorCode::MissingClientId => "missingClientId",
            ErrorCode::AppMisconfigured => "appMisconfigured",
            ErrorCode::AppNotApproved => "appNotApproved",
            ErrorCode::LoginDeclined => "loginDeclined",
            ErrorCode::LoginExpired => "loginExpired",
            ErrorCode::DeviceCodeExpired => "deviceCodeExpired",
            ErrorCode::BrowserLoginTimedOut => "browserLoginTimedOut",
            ErrorCode::LoopbackUnavailable => "loopbackUnavailable",
            ErrorCode::BrowserLoginFailed => "browserLoginFailed",
            ErrorCode::NoXboxAccount => "noXboxAccount",
            ErrorCode::XboxRegionBlocked => "xboxRegionBlocked",
            ErrorCode::XboxAdultVerification => "xboxAdultVerification",
            ErrorCode::XboxChildAccount => "xboxChildAccount",
            ErrorCode::XboxRefused => "xboxRefused",
            ErrorCode::GameNotOwned => "gameNotOwned",
            ErrorCode::NoProfile => "noProfile",
            ErrorCode::AccountNotFound => "accountNotFound",
            ErrorCode::NoAccount => "noAccount",
            ErrorCode::Cancelled => "cancelled",
            ErrorCode::FileSystem => "fileSystem",
            ErrorCode::OpenFailed => "openFailed",
            ErrorCode::InstanceNotFound => "instanceNotFound",
            ErrorCode::InstanceExists => "instanceExists",
            ErrorCode::InstanceFormatTooNew => "instanceFormatTooNew",
            ErrorCode::InvalidInstanceName => "invalidInstanceName",
            ErrorCode::InstanceUnreadable => "instanceUnreadable",
            ErrorCode::InstanceLinked => "instanceLinked",
            ErrorCode::RecycleBinPartial => "recycleBinPartial",
            ErrorCode::RecycleBinFailed => "recycleBinFailed",
            ErrorCode::RecycleBinUnconfirmed => "recycleBinUnconfirmed",
            ErrorCode::RecycleBinUnknown => "recycleBinUnknown",
            ErrorCode::RecycleBinPending => "recycleBinPending",
            ErrorCode::InstanceRunning => "instanceRunning",
            ErrorCode::Busy => "busy",
        }
    }

    const ALL_CODES: [ErrorCode; 53] = [
        ErrorCode::Internal,
        ErrorCode::InvalidInput,
        ErrorCode::Network,
        ErrorCode::Offline,
        ErrorCode::ServerError,
        ErrorCode::ChecksumMismatch,
        ErrorCode::UnsafeData,
        ErrorCode::UnknownVersion,
        ErrorCode::NoJavaRuntime,
        ErrorCode::OfflineAccountBlocked,
        ErrorCode::NoLoaderVersions,
        ErrorCode::UnknownLoaderVersion,
        ErrorCode::LoaderUnsupported,
        ErrorCode::InstallerInvalid,
        ErrorCode::InstallFailed,
        ErrorCode::InstallStepFailed,
        ErrorCode::NativesInUse,
        ErrorCode::LockTimeout,
        ErrorCode::CredentialStore,
        ErrorCode::MissingClientId,
        ErrorCode::AppMisconfigured,
        ErrorCode::AppNotApproved,
        ErrorCode::LoginDeclined,
        ErrorCode::LoginExpired,
        ErrorCode::DeviceCodeExpired,
        ErrorCode::BrowserLoginTimedOut,
        ErrorCode::LoopbackUnavailable,
        ErrorCode::BrowserLoginFailed,
        ErrorCode::NoXboxAccount,
        ErrorCode::XboxRegionBlocked,
        ErrorCode::XboxAdultVerification,
        ErrorCode::XboxChildAccount,
        ErrorCode::XboxRefused,
        ErrorCode::GameNotOwned,
        ErrorCode::NoProfile,
        ErrorCode::AccountNotFound,
        ErrorCode::NoAccount,
        ErrorCode::Cancelled,
        ErrorCode::FileSystem,
        ErrorCode::OpenFailed,
        ErrorCode::InstanceNotFound,
        ErrorCode::InstanceExists,
        ErrorCode::InstanceFormatTooNew,
        ErrorCode::InvalidInstanceName,
        ErrorCode::InstanceUnreadable,
        ErrorCode::InstanceLinked,
        ErrorCode::RecycleBinPartial,
        ErrorCode::RecycleBinFailed,
        ErrorCode::RecycleBinUnconfirmed,
        ErrorCode::RecycleBinUnknown,
        ErrorCode::RecycleBinPending,
        ErrorCode::InstanceRunning,
        ErrorCode::Busy,
    ];

    #[test]
    fn every_wire_code_is_pinned() {
        for code in ALL_CODES {
            assert_eq!(
                serde_json::to_string(&code).unwrap(),
                format!("\"{}\"", wire(code))
            );
        }
        // The list is complete: the generated TypeScript union names exactly these codes.
        let ts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../src/ipc/bindings/ErrorCode.ts"
        ))
        .unwrap();
        let union = &ts[ts.find("export type ErrorCode").unwrap()..];
        let mut in_ts: Vec<&str> = union.split('"').skip(1).step_by(2).collect();
        let mut listed: Vec<&str> = ALL_CODES.iter().map(|c| wire(*c)).collect();
        in_ts.sort_unstable();
        listed.sort_unstable();
        assert_eq!(in_ts, listed);
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
