//! Minecraft services: login with Xbox, ownership check, player profile.

use super::xbox::XboxToken;
use super::{http_error, AuthError, MicrosoftAuth};
use crate::secret::Secret;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Clone)]
pub struct MinecraftToken {
    pub token: Secret,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize)]
struct LoginResponse {
    access_token: Secret,
    expires_in: u64,
}

#[derive(Deserialize)]
struct Entitlements {
    #[serde(default)]
    items: Vec<Entitlement>,
}

#[derive(Deserialize)]
struct Entitlement {
    name: Option<String>,
}

/// The id becomes a credential-store key and the name lands in launch
/// arguments, so both are checked strictly.
fn valid_profile(p: &Profile) -> bool {
    p.id.len() == 32
        && p.id.bytes().all(|b| b.is_ascii_hexdigit())
        && (1..=16).contains(&p.name.len())
        && p.name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Game Pass often returns empty entitlements, so a profile wins.
pub(crate) fn ownership(owns: bool, profile: Option<Profile>) -> Result<Profile> {
    match (profile, owns) {
        (Some(p), _) => Ok(p),
        (None, true) => Err(AuthError::NoProfile.into()),
        (None, false) => Err(AuthError::GameNotOwned.into()),
    }
}

impl MicrosoftAuth {
    /// Exchange an XSTS token for a Minecraft services access token.
    pub async fn minecraft_login(&self, xsts: &XboxToken) -> Result<MinecraftToken> {
        let url = format!("{}/authentication/login_with_xbox", self.ep.minecraft);
        let body = json!({
            "identityToken": format!("XBL3.0 x={};{}", xsts.uhs, xsts.token.expose()),
        });
        let resp = self.post_json(&url, &body, &[]).await?;
        let status = resp.status();
        if status.as_u16() == 403 {
            return Err(AuthError::AppNotApproved.into());
        }
        if !status.is_success() {
            return Err(Error::Status {
                status: status.as_u16(),
                url,
            });
        }
        let r: LoginResponse = resp.json().await.map_err(http_error(&url))?;
        Ok(MinecraftToken {
            token: r.access_token,
            expires_in: r.expires_in,
        })
    }

    /// Whether the account owns the Java Edition (entitlement signatures are
    /// not verified).
    pub async fn owns_game(&self, mc: &Secret) -> Result<bool> {
        let url = format!("{}/entitlements/mcstore", self.ep.minecraft);
        let resp = self.get_bearer(&url, mc).await?;
        if !resp.status().is_success() {
            return Err(Error::Status {
                status: resp.status().as_u16(),
                url,
            });
        }
        let e: Entitlements = resp.json().await.map_err(http_error(&url))?;
        Ok(e.items.iter().any(|i| {
            matches!(
                i.name.as_deref(),
                Some("game_minecraft" | "product_minecraft")
            )
        }))
    }

    /// The player profile, or `None` when the account has none yet.
    pub async fn profile(&self, mc: &Secret) -> Result<Option<Profile>> {
        let url = format!("{}/minecraft/profile", self.ep.minecraft);
        let resp = self.get_bearer(&url, mc).await?;
        let status = resp.status();
        if status.as_u16() == 404 {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(Error::Status {
                status: status.as_u16(),
                url,
            });
        }
        let bad = || Error::Auth(AuthError::BadResponse("Minecraft profile"));
        let p: Profile = resp.json().await.map_err(|_| bad())?;
        if valid_profile(&p) {
            Ok(Some(p))
        } else {
            Err(bad())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AuthEndpoints, AuthError};
    use crate::secret::Secret;
    use crate::Error;
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const UUID: &str = "069a79f444e94726a5befca90e38aaf5";

    fn auth(server: &MockServer) -> MicrosoftAuth {
        MicrosoftAuth::new(
            "test-id".into(),
            AuthEndpoints {
                minecraft: server.uri(),
                ..AuthEndpoints::default()
            },
        )
    }

    fn xsts() -> XboxToken {
        XboxToken {
            token: Secret::new("xsts"),
            uhs: "uhs1".into(),
            xid: None,
        }
    }

    #[tokio::test]
    async fn login_builds_xbl3_identity_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/authentication/login_with_xbox"))
            .and(body_json(json!({"identityToken": "XBL3.0 x=uhs1;xsts"})))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": "mc-secret",
                "expires_in": 86400,
                "token_type": "Bearer",
                "username": "x"
            })))
            .expect(1)
            .mount(&server)
            .await;
        let t = auth(&server).minecraft_login(&xsts()).await.unwrap();
        assert_eq!(t.expires_in, 86400);
        assert_eq!(t.token.expose(), "mc-secret");
        assert!(!format!("{t:?}").contains("mc-secret"));
    }

    #[tokio::test]
    async fn login_403_means_app_not_approved() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(403).set_body_json(json!({
                "path": "/authentication/login_with_xbox",
                "errorMessage": "Invalid app registration"
            })))
            .mount(&server)
            .await;
        let err = auth(&server).minecraft_login(&xsts()).await.unwrap_err();
        assert!(
            matches!(err, Error::Auth(AuthError::AppNotApproved)),
            "{err}"
        );
        assert!(err.to_string().contains("not approved by Mojang yet"));
    }

    #[tokio::test]
    async fn login_other_failure_is_a_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let err = auth(&server).minecraft_login(&xsts()).await.unwrap_err();
        assert!(matches!(err, Error::Status { status: 500, .. }), "{err}");
    }

    #[tokio::test]
    async fn entitlements_and_profile() {
        let mc = Secret::new("mc");

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/entitlements/mcstore"))
            .and(header("authorization", "Bearer mc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "items": [{"name": "product_minecraft"}, {"name": "game_minecraft"}]
            })))
            .expect(1)
            .mount(&server)
            .await;
        assert!(auth(&server).owns_game(&mc).await.unwrap());

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/entitlements/mcstore"))
            .and(header("authorization", "Bearer mc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"items": []})))
            .expect(1)
            .mount(&server)
            .await;
        assert!(!auth(&server).owns_game(&mc).await.unwrap());

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/minecraft/profile"))
            .and(header("authorization", "Bearer mc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": UUID,
                "name": "Notch",
                "skins": [],
                "capes": []
            })))
            .expect(1)
            .mount(&server)
            .await;
        let p = auth(&server).profile(&mc).await.unwrap().unwrap();
        assert_eq!(p.id, UUID);
        assert_eq!(p.name, "Notch");

        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/minecraft/profile"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        assert_eq!(auth(&server).profile(&mc).await.unwrap(), None);
    }

    #[tokio::test]
    async fn entitlements_error_status_is_a_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let err = auth(&server)
            .owns_game(&Secret::new("mc"))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Status { status: 401, .. }), "{err}");
    }

    #[tokio::test]
    async fn profile_with_unsafe_fields_is_a_bad_response() {
        let cases = [
            (UUID, "../x"),
            (UUID, ""),
            (UUID, "seventeen_chars_xx"),
            ("069a79f4-44e9-4726-a5be-fca90e38aaf5", "Notch"),
            ("../../etc/passwd000000000000000", "Notch"),
            ("069a79f444e94726a5befca90e38aaf", "Notch"),
            ("069a79f444e94726a5befca90e38aafg", "Notch"),
        ];
        for (id, name) in cases {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .respond_with(
                    ResponseTemplate::new(200).set_body_json(json!({"id": id, "name": name})),
                )
                .mount(&server)
                .await;
            let err = auth(&server).profile(&Secret::new("mc")).await.unwrap_err();
            assert!(
                matches!(
                    err,
                    Error::Auth(AuthError::BadResponse("Minecraft profile"))
                ),
                "{id} {name}: {err}"
            );
        }
    }

    #[test]
    fn ownership_table() {
        let p = || Profile {
            id: UUID.into(),
            name: "Notch".into(),
        };
        assert_eq!(ownership(true, Some(p())).unwrap(), p());
        assert_eq!(ownership(false, Some(p())).unwrap(), p());
        assert!(matches!(
            ownership(true, None),
            Err(Error::Auth(AuthError::NoProfile))
        ));
        assert!(matches!(
            ownership(false, None),
            Err(Error::Auth(AuthError::GameNotOwned))
        ));
    }
}
