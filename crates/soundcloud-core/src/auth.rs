use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const GOOGLE_AUTHORIZE_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const GOOGLE_REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
pub const YOUTUBE_READONLY_SCOPE: &str = "https://www.googleapis.com/auth/youtube.readonly";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthEndpoints {
    pub authorize_url: String,
    pub token_url: String,
    pub revoke_url: String,
}

impl Default for OAuthEndpoints {
    fn default() -> Self {
        Self {
            authorize_url: GOOGLE_AUTHORIZE_URL.to_string(),
            token_url: GOOGLE_TOKEN_URL.to_string(),
            revoke_url: GOOGLE_REVOKE_URL.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn from_verifier(verifier: impl Into<String>) -> Self {
        let verifier = verifier.into();
        let challenge = pkce_challenge(&verifier);
        Self {
            verifier,
            challenge,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    pub scope: Option<String>,
    pub token_type: Option<String>,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid authorization URL")]
    InvalidAuthorizationUrl(#[source] reqwest::Error),
    #[error("token request failed")]
    TokenRequest(#[source] reqwest::Error),
    #[error("token revocation failed")]
    RevocationRequest(#[source] reqwest::Error),
}

#[derive(Debug, Clone)]
pub struct GoogleAuthClient {
    http: reqwest::Client,
    credentials: OAuthCredentials,
    endpoints: OAuthEndpoints,
}

impl GoogleAuthClient {
    pub fn new(credentials: OAuthCredentials) -> Self {
        Self::with_endpoints(credentials, OAuthEndpoints::default())
    }

    pub fn with_endpoints(credentials: OAuthCredentials, endpoints: OAuthEndpoints) -> Self {
        Self {
            http: reqwest::Client::new(),
            credentials,
            endpoints,
        }
    }

    pub fn authorization_url(&self, state: &str, pkce: &Pkce) -> Result<reqwest::Url, AuthError> {
        self.http
            .get(&self.endpoints.authorize_url)
            .query(&[
                ("client_id", self.credentials.client_id.as_str()),
                ("redirect_uri", self.credentials.redirect_uri.as_str()),
                ("response_type", "code"),
                ("scope", YOUTUBE_READONLY_SCOPE),
                ("code_challenge", pkce.challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("access_type", "offline"),
                ("state", state),
            ])
            .build()
            .map(|req| req.url().clone())
            .map_err(AuthError::InvalidAuthorizationUrl)
    }

    pub async fn exchange_authorization_code(
        &self,
        code: &str,
        pkce: &Pkce,
    ) -> Result<TokenResponse, AuthError> {
        let response = self
            .http
            .post(&self.endpoints.token_url)
            .header("Accept", "application/json; charset=utf-8")
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", self.credentials.client_id.as_str()),
                ("client_secret", self.credentials.client_secret.as_str()),
                ("redirect_uri", self.credentials.redirect_uri.as_str()),
                ("code_verifier", pkce.verifier.as_str()),
                ("code", code),
            ])
            .send()
            .await
            .map_err(AuthError::TokenRequest)?
            .error_for_status()
            .map_err(AuthError::TokenRequest)?;

        response.json().await.map_err(AuthError::TokenRequest)
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse, AuthError> {
        let response = self
            .http
            .post(&self.endpoints.token_url)
            .header("Accept", "application/json; charset=utf-8")
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", self.credentials.client_id.as_str()),
                ("client_secret", self.credentials.client_secret.as_str()),
                ("refresh_token", refresh_token),
            ])
            .send()
            .await
            .map_err(AuthError::TokenRequest)?
            .error_for_status()
            .map_err(AuthError::TokenRequest)?;

        response.json().await.map_err(AuthError::TokenRequest)
    }

    pub async fn revoke_token(&self, token: &str) -> Result<(), AuthError> {
        self.http
            .post(&self.endpoints.revoke_url)
            .query(&[("token", token)])
            .send()
            .await
            .map_err(AuthError::RevocationRequest)?
            .error_for_status()
            .map_err(AuthError::RevocationRequest)?;
        Ok(())
    }
}

// ── SoundCloud auth ─────────────────────────────────────────────────────────

pub const SOUNDCLOUD_AUTHORIZE_URL: &str = "https://secure.soundcloud.com";
pub const SOUNDCLOUD_TOKEN_URL: &str = "https://api.soundcloud.com/oauth2/token";

#[derive(Debug, Clone)]
pub struct SoundCloudAuthClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

impl SoundCloudAuthClient {
    pub fn new(client_id: String, client_secret: String, redirect_uri: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            client_id,
            client_secret,
            redirect_uri,
        }
    }

    pub fn authorization_url(&self, state: &str, pkce: &Pkce) -> Result<reqwest::Url, AuthError> {
        self.http
            .get(SOUNDCLOUD_AUTHORIZE_URL)
            .query(&[
                ("client_id", self.client_id.as_str()),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("response_type", "code"),
                ("code_challenge", pkce.challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("state", state),
            ])
            .build()
            .map(|req| req.url().clone())
            .map_err(AuthError::InvalidAuthorizationUrl)
    }

    pub async fn exchange_authorization_code(
        &self,
        code: &str,
        pkce: &Pkce,
    ) -> Result<TokenResponse, AuthError> {
        let response = self
            .http
            .post(SOUNDCLOUD_TOKEN_URL)
            .header("Accept", "application/json; charset=utf-8")
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("code_verifier", pkce.verifier.as_str()),
                ("code", code),
            ])
            .send()
            .await
            .map_err(AuthError::TokenRequest)?
            .error_for_status()
            .map_err(AuthError::TokenRequest)?;
        response.json().await.map_err(AuthError::TokenRequest)
    }
}

pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> GoogleAuthClient {
        GoogleAuthClient::new(OAuthCredentials {
            client_id: "client-id.apps.googleusercontent.com".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: "meowify://google/callback".to_string(),
        })
    }

    #[test]
    fn pkce_challenge_matches_rfc_example() {
        let challenge = pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
        assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn builds_documented_authorization_url() {
        let pkce = Pkce::from_verifier("verifier");
        let url = client().authorization_url("state-123", &pkce).unwrap();

        assert_eq!(
            url.as_str().split('?').next().unwrap(),
            GOOGLE_AUTHORIZE_URL
        );
        assert!(url.query().unwrap().contains("response_type=code"));
        assert!(url.query().unwrap().contains("code_challenge_method=S256"));
        assert!(url.query().unwrap().contains("state=state-123"));
        assert!(url.query().unwrap().contains("access_type=offline"));
    }

    #[test]
    fn parses_token_response() {
        let token: TokenResponse = serde_json::from_str(
            r#"{
                "access_token": "ya29.access",
                "refresh_token": "1//refresh",
                "expires_in": 3600,
                "scope": "https://www.googleapis.com/auth/youtube.readonly",
                "token_type": "Bearer"
            }"#,
        )
        .unwrap();

        assert_eq!(token.access_token, "ya29.access");
        assert_eq!(token.refresh_token.as_deref(), Some("1//refresh"));
        assert_eq!(token.expires_in, 3600);
    }
}
