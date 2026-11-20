/// Errors of the sign-in chain. Messages never contain tokens or
/// Microsoft's free-text error descriptions.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("no Microsoft client id configured; set HENGST_MS_CLIENT_ID (the official id is not published yet)")]
    MissingClientId,
    #[error("the sign-in code expired before it was used; run \"hengst login\" again")]
    DeviceCodeExpired,
    #[error("the sign-in was declined in the browser")]
    LoginDeclined,
    #[error("Microsoft rejected the client id ({code}); the Azure app must allow personal accounts and public client flows")]
    AppMisconfigured { code: String },
    #[error("the saved login for {name} is no longer valid; run \"hengst login\" again")]
    LoginExpired { name: String },
    #[error("no Xbox account")]
    NoXboxAccount,
    #[error("Xbox is not available in this region")]
    XboxRegionBlocked,
    #[error("the Xbox account needs adult verification")]
    XboxAdultVerification,
    #[error("the Xbox account is a child account")]
    XboxChildAccount,
    #[error("Xbox refused the sign-in (code {code})")]
    XboxRefused { code: u64 },
    #[error("the launcher app is not approved for the Minecraft services")]
    AppNotApproved,
    #[error("this account does not own the game")]
    GameNotOwned,
    #[error("this account has no Minecraft profile")]
    NoProfile,
    #[error("no saved account matches {selector}")]
    AccountNotFound { selector: String },
    #[error("no account is signed in; run \"hengst login\"")]
    NoAccount,
    #[error("unexpected response from {0}")]
    BadResponse(&'static str),
}
