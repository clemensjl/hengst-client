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
    #[error("this Microsoft account has no Xbox profile yet; sign in once at https://www.xbox.com to create one, then try again")]
    NoXboxAccount,
    #[error("Xbox Live is not available in this account's country or region")]
    XboxRegionBlocked,
    #[error("this account needs adult verification on the Xbox website before it can sign in")]
    XboxAdultVerification,
    #[error("this is a child account; an adult must add it to a Microsoft family group before it can sign in")]
    XboxChildAccount,
    #[error("Xbox Live refused the sign-in (XErr {code})")]
    XboxRefused { code: u64 },
    #[error("the Minecraft services rejected this launcher's Azure app (HTTP 403): the client id is not approved by Mojang yet")]
    AppNotApproved,
    #[error("this Microsoft account does not own Minecraft: Java Edition")]
    GameNotOwned,
    #[error("this account owns the game but has no player name yet; choose one on minecraft.net, then try again")]
    NoProfile,
    #[error("no account matches \"{selector}\"; run \"hengst accounts\" to list them")]
    AccountNotFound { selector: String },
    #[error("no Microsoft account signed in; run \"hengst login\" first")]
    NoAccount,
    #[error("unexpected response from {0}")]
    BadResponse(&'static str),
}
