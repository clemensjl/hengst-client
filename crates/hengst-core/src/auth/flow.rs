//! The full sign-in chain: Microsoft tokens to a Minecraft login session.

use super::minecraft::{ownership, Profile};
use super::msa::MsaTokens;
use super::store::{AccountStore, SecretKind, SecretStore, StoredAccount};
use super::{AuthError, MicrosoftAuth};
use crate::args::Account;
use crate::secret::Secret;
use crate::{Error, Result};

/// Refresh when less than this many seconds of the Minecraft token remain.
pub(crate) const REFRESH_MARGIN_SECS: u64 = 3600;

pub fn needs_refresh(account: &StoredAccount, now: u64) -> bool {
    account.minecraft_expires_at <= now.saturating_add(REFRESH_MARGIN_SECS)
}

/// Result of a completed login. Tokens are `Secret`, so `Debug` is safe.
#[derive(Debug, Clone)]
pub struct LoginSession {
    pub profile: Profile,
    pub xuid: String,
    pub minecraft_token: Secret,
    /// Unix seconds.
    pub minecraft_expires_at: u64,
    pub refresh_token: Secret,
}

impl LoginSession {
    /// The account to start the game with.
    pub(crate) fn to_account(&self) -> Account {
        Account::microsoft(
            self.profile.name.clone(),
            self.profile.id.clone(),
            self.minecraft_token.clone(),
            self.xuid.clone(),
        )
    }
}

/// Only a dead network or a failing server may fall back to the stored token.
fn is_transient(e: &Error) -> bool {
    match e {
        Error::Http { .. } => true,
        Error::Status { status, .. } => *status >= 500 || *status == 429,
        _ => false,
    }
}

/// The account to start the game with. Renews the saved login first when the
/// Minecraft token is about to expire and stores the rotated refresh token.
pub async fn launch_account<S: SecretStore>(
    store: &AccountStore<S>,
    auth: Option<&MicrosoftAuth>,
    selector: Option<&str>,
    now: u64,
) -> Result<Account> {
    launch_account_with(store, auth, selector, now, |session| {
        store.save_session(session).map(|_| ())
    })
    .await
}

/// `launch_account` that hands a renewed session to `save` instead of storing it itself. `save`
/// runs synchronously after the renewal answered (no await inside it), so the caller can save
/// under its own lock and refuse a session whose account was removed meanwhile (its error is
/// returned and nothing is launched).
pub async fn launch_account_with<S: SecretStore>(
    store: &AccountStore<S>,
    auth: Option<&MicrosoftAuth>,
    selector: Option<&str>,
    now: u64,
    save: impl FnOnce(&LoginSession) -> Result<()>,
) -> Result<Account> {
    let stored = store.find(selector)?;
    let saved = |stored: &StoredAccount| -> Result<Account> {
        Ok(Account::microsoft(
            stored.name.clone(),
            stored.uuid.clone(),
            store.secret(stored, SecretKind::Minecraft)?,
            stored.xuid.clone(),
        ))
    };
    if !needs_refresh(&stored, now) {
        return saved(&stored);
    }
    let auth = auth.ok_or(Error::Auth(AuthError::MissingClientId))?;
    let refresh = store.secret(&stored, SecretKind::Refresh)?;
    match auth.refresh_session(&refresh, now).await {
        Ok(session) => {
            save(&session)?;
            Ok(session.to_account())
        }
        Err(Error::Auth(AuthError::LoginExpired { .. })) => {
            Err(Error::Auth(AuthError::LoginExpired { name: stored.name }))
        }
        Err(e) if is_transient(&e) && stored.minecraft_expires_at > now => saved(&stored),
        Err(e) => Err(e),
    }
}

impl MicrosoftAuth {
    /// Xbox user token, XSTS, Minecraft login, ownership and profile.
    pub async fn complete_login(&self, msa: MsaTokens, now: u64) -> Result<LoginSession> {
        let user = self.xbox_user_token(&msa.access_token).await?;
        let xsts = self.xsts_token(&user).await?;
        let mc = self.minecraft_login(&xsts).await?;
        let owns = self.owns_game(&mc.token).await?;
        let profile = self.profile(&mc.token).await?;
        let mut profile = ownership(owns, profile)?;
        profile.id.make_ascii_lowercase();
        Ok(LoginSession {
            profile,
            xuid: xsts.xid.unwrap_or_else(|| "0".to_string()),
            minecraft_token: mc.token,
            minecraft_expires_at: now.saturating_add(mc.expires_in),
            refresh_token: msa.refresh_token,
        })
    }

    /// Renew the whole session from a saved Microsoft refresh token.
    pub async fn refresh_session(&self, refresh: &Secret, now: u64) -> Result<LoginSession> {
        let msa = self.refresh_msa(refresh).await?;
        self.complete_login(msa, now).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::minecraft::Profile;
    use crate::auth::store::MemorySecretStore;

    fn store_with_old_token(
        expires_at: u64,
    ) -> (tempfile::TempDir, AccountStore<MemorySecretStore>) {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::new(dir.path().join("a.json"), MemorySecretStore::default());
        store
            .save_session(&LoginSession {
                profile: Profile {
                    id: "0123456789abcdef0123456789abcdef".into(),
                    name: "Steve".into(),
                },
                xuid: "2535".into(),
                minecraft_token: Secret::new("mc-old"),
                minecraft_expires_at: expires_at,
                refresh_token: Secret::new("rt-1"),
            })
            .unwrap();
        (dir, store)
    }

    #[tokio::test]
    async fn fresh_launch_hands_over_the_stored_token_and_xuid() {
        let (_d, store) = store_with_old_token(10_000);
        let a = launch_account(&store, None, None, 0).await.unwrap();
        assert_eq!(a.access_token().expose(), "mc-old");
        assert_eq!(a.xuid(), "2535");
    }

    #[test]
    fn session_converts_to_a_microsoft_account() {
        let session = LoginSession {
            profile: Profile {
                id: "0123456789abcdef0123456789abcdef".into(),
                name: "Notch".into(),
            },
            xuid: "2535".into(),
            minecraft_token: Secret::new("mc-new"),
            minecraft_expires_at: 1,
            refresh_token: Secret::new("rt"),
        };
        let a = session.to_account();
        assert_eq!(a.user_type(), "msa");
        assert_eq!(a.name(), "Notch");
        assert_eq!(a.uuid(), "0123456789abcdef0123456789abcdef");
        assert_eq!(a.access_token().expose(), "mc-new");
        assert_eq!(a.xuid(), "2535");
    }
}
