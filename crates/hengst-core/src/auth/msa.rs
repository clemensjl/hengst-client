//! Microsoft identity platform: device code flow and token refresh.

use super::{http_error, AuthError, MicrosoftAuth, SCOPE};
use crate::secret::Secret;
use crate::{Error, Result};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
pub struct DeviceCode {
    pub device_code: Secret,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug)]
pub struct MsaTokens {
    pub access_token: Secret,
    pub refresh_token: Secret,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Secret,
    refresh_token: Secret,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

pub(crate) fn next_interval(current: Duration, error: &str) -> Duration {
    if error == "slow_down" {
        current + Duration::from_secs(5)
    } else {
        current
    }
}

/// Read the `error` code of a failed token response (HTTP 400, or 401 as Entra
/// answers `invalid_client`). The free-text
/// `error_description` is deliberately ignored.
async fn error_code(resp: reqwest::Response, url: &str) -> Result<String> {
    let status = resp.status().as_u16();
    let body = resp.bytes().await.map_err(http_error(url))?;
    match serde_json::from_slice::<ErrorResponse>(&body) {
        Ok(e) if status == 400 || status == 401 => Ok(e.error),
        _ => Err(Error::Status {
            url: url.to_string(),
            status,
        }),
    }
}

/// `invalid_client` / `unauthorized_client` mean a misconfigured app;
/// everything else falls back to `other`.
fn map_client_error(code: String, other: Error) -> Error {
    match code.as_str() {
        "invalid_client" | "unauthorized_client" => AuthError::AppMisconfigured { code }.into(),
        _ => other,
    }
}

async fn parse_tokens(resp: reqwest::Response, url: &str) -> Result<MsaTokens> {
    let t: TokenResponse = resp.json().await.map_err(http_error(url))?;
    Ok(MsaTokens {
        access_token: t.access_token,
        refresh_token: t.refresh_token,
    })
}

impl MicrosoftAuth {
    pub async fn start_device_login(&self) -> Result<DeviceCode> {
        let url = format!("{}/devicecode", self.ep.msa);
        let resp = self
            .post_form(&url, &[("client_id", &self.client_id), ("scope", SCOPE)])
            .await?;
        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let code = error_code(resp, &url).await?;
            return Err(map_client_error(code, Error::Status { url, status }));
        }
        resp.json().await.map_err(http_error(&url))
    }

    pub async fn poll_device_login(&self, code: &DeviceCode) -> Result<MsaTokens> {
        let url = format!("{}/token", self.ep.msa);
        let deadline = Instant::now() + Duration::from_secs(code.expires_in);
        let mut interval = Duration::from_secs(code.interval);
        loop {
            tokio::time::sleep(interval).await;
            if Instant::now() >= deadline {
                return Err(AuthError::DeviceCodeExpired.into());
            }
            let resp = match self
                .post_form(
                    &url,
                    &[
                        ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                        ("client_id", &self.client_id),
                        ("device_code", code.device_code.expose()),
                    ],
                )
                .await
            {
                Ok(resp) => resp,
                // A dropped connection must not end the login; keep polling.
                Err(Error::Http { .. }) => continue,
                Err(e) => return Err(e),
            };
            if resp.status().is_success() {
                return parse_tokens(resp, &url).await;
            }
            let status = resp.status().as_u16();
            if status >= 500 || status == 429 {
                continue;
            }
            let err = error_code(resp, &url).await?;
            match err.as_str() {
                "authorization_pending" => {}
                "slow_down" => interval = next_interval(interval, &err),
                "authorization_declined" => return Err(AuthError::LoginDeclined.into()),
                "expired_token" => return Err(AuthError::DeviceCodeExpired.into()),
                _ => return Err(map_client_error(err, Error::Status { url, status })),
            }
        }
    }

    pub async fn refresh_msa(&self, refresh: &Secret) -> Result<MsaTokens> {
        let url = format!("{}/token", self.ep.msa);
        let resp = self
            .post_form(
                &url,
                &[
                    ("grant_type", "refresh_token"),
                    ("client_id", &self.client_id),
                    ("refresh_token", refresh.expose()),
                    ("scope", SCOPE),
                ],
            )
            .await?;
        if resp.status().is_success() {
            return parse_tokens(resp, &url).await;
        }
        let status = resp.status().as_u16();
        let err = error_code(resp, &url).await?;
        if err == "invalid_grant" {
            return Err(AuthError::LoginExpired {
                name: String::new(),
            }
            .into());
        }
        Err(map_client_error(err, Error::Status { url, status }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthEndpoints;
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn auth(server: &MockServer) -> MicrosoftAuth {
        MicrosoftAuth::new(
            "test-id".into(),
            AuthEndpoints {
                msa: server.uri(),
                ..AuthEndpoints::default()
            },
        )
    }

    fn code() -> DeviceCode {
        DeviceCode {
            device_code: Secret::new("dc-1"),
            user_code: "ABCD-EFGH".into(),
            verification_uri: "https://microsoft.com/link".into(),
            expires_in: 900,
            interval: 0,
        }
    }

    fn token_ok() -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "at-1", "refresh_token": "rt-1", "expires_in": 3600
        }))
    }

    fn token_err(e: &str) -> ResponseTemplate {
        ResponseTemplate::new(400)
            .set_body_json(json!({"error": e, "error_description": "secret detail"}))
    }

    #[tokio::test]
    async fn device_code_request_uses_consumers_scope() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/devicecode"))
            .and(body_string_contains("scope=XboxLive.signin+offline_access"))
            .and(body_string_contains("client_id=test-id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code": "dc-1", "user_code": "ABCD-EFGH",
                "verification_uri": "https://microsoft.com/link",
                "expires_in": 900, "interval": 0
            })))
            .expect(1)
            .mount(&server)
            .await;
        let dc = auth(&server).start_device_login().await.unwrap();
        assert_eq!(dc.user_code, "ABCD-EFGH");
        assert_eq!(dc.verification_uri, "https://microsoft.com/link");
        assert_eq!(dc.device_code.expose(), "dc-1");
    }

    #[tokio::test]
    async fn polling_waits_through_pending_then_returns_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("device_code=dc-1"))
            .and(body_string_contains("client_id=test-id"))
            .respond_with(token_err("authorization_pending"))
            .up_to_n_times(2)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(token_ok())
            .with_priority(2)
            .mount(&server)
            .await;
        let tokens = auth(&server).poll_device_login(&code()).await.unwrap();
        assert_eq!(tokens.access_token.expose(), "at-1");
        assert_eq!(tokens.refresh_token.expose(), "rt-1");
        assert_eq!(server.received_requests().await.unwrap().len(), 3);
    }

    async fn poll_error(e: &str) -> Error {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(token_err(e))
            .mount(&server)
            .await;
        auth(&server).poll_device_login(&code()).await.unwrap_err()
    }

    #[tokio::test]
    async fn declined_and_expired_codes_map_to_clear_errors() {
        assert!(matches!(
            poll_error("authorization_declined").await,
            Error::Auth(AuthError::LoginDeclined)
        ));
        assert!(matches!(
            poll_error("expired_token").await,
            Error::Auth(AuthError::DeviceCodeExpired)
        ));
        match poll_error("unauthorized_client").await {
            Error::Auth(AuthError::AppMisconfigured { code }) => {
                assert_eq!(code, "unauthorized_client")
            }
            other => panic!("unexpected: {other}"),
        }
        let unknown = poll_error("server_error").await;
        assert!(
            matches!(unknown, Error::Status { status: 400, .. }),
            "{unknown}"
        );
        assert!(!unknown.to_string().contains("secret detail"));
    }

    #[tokio::test]
    async fn polling_gives_up_after_expires_in() {
        let server = MockServer::start().await;
        let mut dc = code();
        dc.expires_in = 0;
        let err = auth(&server).poll_device_login(&dc).await.unwrap_err();
        assert!(matches!(err, Error::Auth(AuthError::DeviceCodeExpired)));
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn polling_survives_a_server_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(503))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(token_ok())
            .with_priority(2)
            .mount(&server)
            .await;
        let tokens = auth(&server).poll_device_login(&code()).await.unwrap();
        assert_eq!(tokens.access_token.expose(), "at-1");
        assert_eq!(server.received_requests().await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn polling_survives_rate_limiting() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(429))
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(token_ok())
            .with_priority(2)
            .mount(&server)
            .await;
        assert!(auth(&server).poll_device_login(&code()).await.is_ok());
    }

    #[tokio::test]
    async fn polling_through_endless_server_errors_ends_with_expiry() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let mut dc = code();
        dc.expires_in = 1;
        let err = auth(&server).poll_device_login(&dc).await.unwrap_err();
        assert!(matches!(err, Error::Auth(AuthError::DeviceCodeExpired)));
        assert!(!server.received_requests().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_client_with_401_maps_to_app_misconfigured() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(401)
                    .set_body_json(json!({"error": "invalid_client", "error_description": "x"})),
            )
            .mount(&server)
            .await;
        match auth(&server).poll_device_login(&code()).await.unwrap_err() {
            Error::Auth(AuthError::AppMisconfigured { code }) => assert_eq!(code, "invalid_client"),
            other => panic!("unexpected: {other}"),
        }
    }

    #[test]
    fn slow_down_adds_five_seconds() {
        assert_eq!(
            next_interval(Duration::from_secs(5), "slow_down"),
            Duration::from_secs(10)
        );
        assert_eq!(
            next_interval(Duration::from_secs(5), "authorization_pending"),
            Duration::from_secs(5)
        );
    }

    #[tokio::test]
    async fn refresh_sends_refresh_grant_and_rejects_invalid_grant() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains("refresh_token=old-rt"))
            .and(body_string_contains("scope=XboxLive.signin+offline_access"))
            .respond_with(token_ok())
            .mount(&server)
            .await;
        let tokens = auth(&server)
            .refresh_msa(&Secret::new("old-rt"))
            .await
            .unwrap();
        assert_eq!(tokens.refresh_token.expose(), "rt-1");

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(token_err("invalid_grant"))
            .mount(&server)
            .await;
        let err = auth(&server)
            .refresh_msa(&Secret::new("old-rt"))
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            Error::Auth(AuthError::LoginExpired { ref name }) if name.is_empty()
        ));
    }
}
