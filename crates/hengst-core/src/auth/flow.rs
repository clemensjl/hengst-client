//! The full sign-in chain: Microsoft tokens to a Minecraft login session.

use super::minecraft::{ownership, Profile};
use super::msa::MsaTokens;
use super::MicrosoftAuth;
use crate::args::Account;
use crate::secret::Secret;
use crate::Result;

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
    pub fn to_account(&self) -> Account {
        Account::microsoft(
            self.profile.name.clone(),
            self.profile.id.clone(),
            self.minecraft_token.clone(),
            self.xuid.clone(),
        )
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
