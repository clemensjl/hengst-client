//! Xbox Live user authentication and XSTS authorization.

use super::{AuthError, MicrosoftAuth};
use crate::secret::Secret;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::json;

/// Relying party the XSTS token is issued for.
const MINECRAFT_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";

#[derive(Debug, Clone)]
pub struct XboxToken {
    pub token: Secret,
    pub uhs: String,
    pub xid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XboxResponse {
    token: Option<Secret>,
    display_claims: Option<DisplayClaims>,
}

#[derive(Deserialize)]
struct DisplayClaims {
    #[serde(default)]
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: Option<String>,
    xid: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct XstsError {
    x_err: u64,
}

pub(crate) fn xsts_error(xerr: u64) -> AuthError {
    match xerr {
        2148916233 => AuthError::NoXboxAccount,
        2148916235 => AuthError::XboxRegionBlocked,
        2148916236 | 2148916237 => AuthError::XboxAdultVerification,
        2148916238 => AuthError::XboxChildAccount,
        code => AuthError::XboxRefused { code },
    }
}

fn parse_token(resp: XboxResponse) -> Result<XboxToken> {
    let bad = || Error::Auth(AuthError::BadResponse("Xbox Live"));
    let token = resp.token.ok_or_else(bad)?;
    let xui = resp
        .display_claims
        .and_then(|c| c.xui.into_iter().next())
        .ok_or_else(bad)?;
    Ok(XboxToken {
        token,
        uhs: xui.uhs.ok_or_else(bad)?,
        xid: xui.xid,
    })
}

impl MicrosoftAuth {
    /// Exchange a Microsoft access token for an Xbox Live user token.
    pub async fn xbox_user_token(&self, msa_access: &Secret) -> Result<XboxToken> {
        let url = format!("{}/user/authenticate", self.ep.xbox_user);
        let body = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={}", msa_access.expose()),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        let resp = self
            .post_json(&url, &body, &[("x-xbl-contract-version", "1")])
            .await?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                status: resp.status().as_u16(),
                url,
            });
        }
        parse_token(resp.json().await.map_err(super::http_error(&url))?)
    }

    /// Exchange an Xbox Live user token for an XSTS token for Minecraft.
    pub async fn xsts_token(&self, user: &XboxToken) -> Result<XboxToken> {
        let url = format!("{}/xsts/authorize", self.ep.xsts);
        let body = json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [user.token.expose()],
            },
            "RelyingParty": MINECRAFT_RELYING_PARTY,
            "TokenType": "JWT",
        });
        let resp = self.post_json(&url, &body, &[]).await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let bytes = resp.bytes().await.map_err(super::http_error(&url))?;
            return match serde_json::from_slice::<XstsError>(&bytes) {
                Ok(e) if status == 401 => Err(xsts_error(e.x_err).into()),
                _ => Err(Error::Status { url, status }),
            };
        }
        parse_token(resp.json().await.map_err(super::http_error(&url))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthEndpoints;
    use crate::secret::Secret;
    use crate::Error;
    use serde_json::json;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn auth(server: &MockServer) -> MicrosoftAuth {
        MicrosoftAuth::new(
            "test-id".into(),
            AuthEndpoints {
                xbox_user: server.uri(),
                xsts: server.uri(),
                ..AuthEndpoints::default()
            },
        )
    }

    fn user_token() -> XboxToken {
        XboxToken {
            token: Secret::new("xbl"),
            uhs: "uhs1".into(),
            xid: None,
        }
    }

    #[tokio::test]
    async fn user_token_sends_rps_ticket_and_reads_uhs() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/user/authenticate"))
            .and(header("x-xbl-contract-version", "1"))
            .and(header("accept", "application/json"))
            .and(body_partial_json(json!({
                "Properties": {
                    "AuthMethod": "RPS",
                    "SiteName": "user.auth.xboxlive.com",
                    "RpsTicket": "d=msa-token"
                },
                "RelyingParty": "http://auth.xboxlive.com",
                "TokenType": "JWT"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xbl",
                "DisplayClaims": {"xui": [{"uhs": "uhs1"}]}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let t = auth(&server)
            .xbox_user_token(&Secret::new("msa-token"))
            .await
            .unwrap();
        assert_eq!(t.uhs, "uhs1");
        assert_eq!(t.token.expose(), "xbl");
        assert_eq!(t.xid, None);
        assert!(!format!("{t:?}").contains("xbl"));
    }

    #[tokio::test]
    async fn user_token_without_uhs_or_token_is_a_bad_response() {
        for body in [
            json!({"Token": "xbl", "DisplayClaims": {"xui": []}}),
            json!({"DisplayClaims": {"xui": [{"uhs": "uhs1"}]}}),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .respond_with(ResponseTemplate::new(200).set_body_json(body))
                .mount(&server)
                .await;
            let err = auth(&server)
                .xbox_user_token(&Secret::new("msa-token"))
                .await
                .unwrap_err();
            assert!(
                matches!(err, Error::Auth(AuthError::BadResponse("Xbox Live"))),
                "{err}"
            );
        }
    }

    #[tokio::test]
    async fn user_token_http_error_is_a_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let err = auth(&server)
            .xbox_user_token(&Secret::new("msa-token"))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Status { status: 500, .. }), "{err}");
    }

    #[tokio::test]
    async fn xsts_targets_minecraft_relying_party() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/xsts/authorize"))
            .and(body_partial_json(json!({
                "RelyingParty": "rp://api.minecraftservices.com/",
                "Properties": {"SandboxId": "RETAIL", "UserTokens": ["xbl"]},
                "TokenType": "JWT"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "Token": "xsts",
                "DisplayClaims": {"xui": [{"uhs": "uhs1", "xid": "2535"}]}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let t = auth(&server).xsts_token(&user_token()).await.unwrap();
        assert_eq!(t.token.expose(), "xsts");
        assert_eq!(t.uhs, "uhs1");
        assert_eq!(t.xid.as_deref(), Some("2535"));
    }

    #[tokio::test]
    async fn xsts_401_maps_known_xerr_codes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "Identity": "0",
                "XErr": 2148916233u64,
                "Message": "",
                "Redirect": "https://start.ui.xboxlive.com/CreateAccount"
            })))
            .mount(&server)
            .await;
        let err = auth(&server).xsts_token(&user_token()).await.unwrap_err();
        assert!(
            matches!(err, Error::Auth(AuthError::NoXboxAccount)),
            "{err}"
        );
    }

    #[tokio::test]
    async fn xsts_401_without_xerr_is_a_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string("nope"))
            .mount(&server)
            .await;
        let err = auth(&server).xsts_token(&user_token()).await.unwrap_err();
        assert!(matches!(err, Error::Status { status: 401, .. }), "{err}");
    }

    #[test]
    fn xerr_table() {
        assert!(matches!(xsts_error(2148916233), AuthError::NoXboxAccount));
        assert!(matches!(
            xsts_error(2148916235),
            AuthError::XboxRegionBlocked
        ));
        assert!(matches!(
            xsts_error(2148916236),
            AuthError::XboxAdultVerification
        ));
        assert!(matches!(
            xsts_error(2148916237),
            AuthError::XboxAdultVerification
        ));
        assert!(matches!(
            xsts_error(2148916238),
            AuthError::XboxChildAccount
        ));
        assert!(xsts_error(42).to_string().contains("XErr 42"));
    }
}
