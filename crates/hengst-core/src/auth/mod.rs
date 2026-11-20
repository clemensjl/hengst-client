//! Microsoft / Xbox / Minecraft sign-in.

pub mod error;
pub mod flow;
pub mod minecraft;
pub mod msa;
pub mod xbox;

pub use error::AuthError;

use crate::download::http_client;
use crate::{Error, Result};

/// Azure application (client) id of Hengst Client. Not a secret.
pub const CLIENT_ID: Option<&str> = Some("7cad9d53-b0ce-44b1-9ef3-afa798da99c2");
pub const CLIENT_ID_ENV: &str = "HENGST_MS_CLIENT_ID";
pub const SCOPE: &str = "XboxLive.signin offline_access";

/// Environment value (trimmed, non-empty) wins over the built-in id.
pub fn resolve_client_id(env: Option<String>, builtin: Option<&str>) -> Result<String> {
    env.map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| builtin.map(str::to_string))
        .ok_or(Error::Auth(AuthError::MissingClientId))
}

pub fn client_id() -> Result<String> {
    resolve_client_id(std::env::var(CLIENT_ID_ENV).ok(), CLIENT_ID)
}

/// Base URLs of every service in the chain; tests point them at a mock.
#[derive(Clone, Debug)]
pub struct AuthEndpoints {
    pub msa: String,
    pub xbox_user: String,
    pub xsts: String,
    pub minecraft: String,
}

impl Default for AuthEndpoints {
    fn default() -> Self {
        AuthEndpoints {
            msa: "https://login.microsoftonline.com/consumers/oauth2/v2.0".into(),
            xbox_user: "https://user.auth.xboxlive.com".into(),
            xsts: "https://xsts.auth.xboxlive.com".into(),
            minecraft: "https://api.minecraftservices.com".into(),
        }
    }
}

pub struct MicrosoftAuth {
    pub(crate) http: reqwest::Client,
    pub(crate) ep: AuthEndpoints,
    pub(crate) client_id: String,
}

impl MicrosoftAuth {
    pub fn new(client_id: String, ep: AuthEndpoints) -> Self {
        MicrosoftAuth {
            http: http_client(),
            ep,
            client_id,
        }
    }

    /// POST a form body; transport errors become `Error::Http`.
    pub(crate) async fn post_form(
        &self,
        url: &str,
        form: &[(&str, &str)],
    ) -> Result<reqwest::Response> {
        self.http
            .post(url)
            .form(form)
            .send()
            .await
            .map_err(http_error(url))
    }

    /// POST a JSON body (asking for a JSON answer) with extra headers;
    /// transport errors become `Error::Http`.
    pub(crate) async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
        headers: &[(&str, &str)],
    ) -> Result<reqwest::Response> {
        let mut req = self
            .http
            .post(url)
            .header("Accept", "application/json")
            .json(body);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        req.send().await.map_err(http_error(url))
    }

    /// GET with a bearer token in the `Authorization` header (never in the
    /// URL); transport errors become `Error::Http`.
    pub(crate) async fn get_bearer(
        &self,
        url: &str,
        token: &crate::secret::Secret,
    ) -> Result<reqwest::Response> {
        self.http
            .get(url)
            .header("Accept", "application/json")
            .bearer_auth(token.expose())
            .send()
            .await
            .map_err(http_error(url))
    }
}

/// Wrap a transport or body-decoding error of a request to `url`.
pub(crate) fn http_error(url: &str) -> impl FnOnce(reqwest::Error) -> Error + '_ {
    move |source| Error::Http {
        url: url.to_string(),
        source,
    }
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_id_env_wins_and_missing_is_a_clear_error() {
        assert_eq!(
            resolve_client_id(Some(" env-id ".into()), Some("built")).unwrap(),
            "env-id"
        );
        assert_eq!(
            resolve_client_id(Some("".into()), Some("built")).unwrap(),
            "built"
        );
        let e = resolve_client_id(None, None).unwrap_err();
        assert!(e.to_string().contains("HENGST_MS_CLIENT_ID"));
        assert!(matches!(e, Error::Auth(AuthError::MissingClientId)));
    }

    #[test]
    fn builtin_client_id_is_present() {
        assert_eq!(
            resolve_client_id(None, CLIENT_ID).unwrap(),
            "7cad9d53-b0ce-44b1-9ef3-afa798da99c2"
        );
    }

    #[test]
    fn default_msa_endpoint_uses_the_consumers_tenant() {
        assert!(AuthEndpoints::default().msa.contains("/consumers/"));
    }
}
