use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SOUNDCLOUD_AUTHORIZE_URL: &str = "https://secure.soundcloud.com/authorize";
pub const SOUNDCLOUD_TOKEN_URL: &str = "https://secure.soundcloud.com/oauth/token";
pub const SOUNDCLOUD_SIGN_OUT_URL: &str = "https://secure.soundcloud.com/sign-out";

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
    pub sign_out_url: String,
}

impl Default for OAuthEndpoints {
    fn default() -> Self {
        Self {
            authorize_url: SOUNDCLOUD_AUTHORIZE_URL.to_string(),
            token_url: SOUNDCLOUD_TOKEN_URL.to_string(),
            sign_out_url: SOUNDCLOUD_SIGN_OUT_URL.to_string(),
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
    pub refresh_token: String,
    pub expires_in: u64,
    pub scope: Option<String>,
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid authorization URL")]
    InvalidAuthorizationUrl(#[source] reqwest::Error),
    #[error("token request failed")]
    TokenRequest(#[source] reqwest::Error),
    #[error("sign out request failed")]
    SignOutRequest(#[source] reqwest::Error),
}

#[derive(Debug, Clone)]
pub struct SoundCloudAuthClient {
    http: reqwest::Client,
    credentials: OAuthCredentials,
    endpoints: OAuthEndpoints,
}

impl SoundCloudAuthClient {
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
                ("code_challenge", pkce.challenge.as_str()),
                ("code_challenge_method", "S256"),
                ("state", state),
            ])
            .build()
            .map(|request| request.url().clone())
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

    pub async fn client_credentials_token(&self) -> Result<TokenResponse, AuthError> {
        let response = self
            .http
            .post(&self.endpoints.token_url)
            .header("Accept", "application/json; charset=utf-8")
            .basic_auth(
                &self.credentials.client_id,
                Some(&self.credentials.client_secret),
            )
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(AuthError::TokenRequest)?
            .error_for_status()
            .map_err(AuthError::TokenRequest)?;

        response.json().await.map_err(AuthError::TokenRequest)
    }

    pub async fn sign_out(&self, access_token: &str) -> Result<(), AuthError> {
        self.http
            .post(&self.endpoints.sign_out_url)
            .json(&serde_json::json!({ "access_token": access_token }))
            .send()
            .await
            .map_err(AuthError::SignOutRequest)?
            .error_for_status()
            .map_err(AuthError::SignOutRequest)?;
        Ok(())
    }
}

pub fn pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> SoundCloudAuthClient {
        SoundCloudAuthClient::new(OAuthCredentials {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: "meowify://soundcloud/callback".to_string(),
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
            SOUNDCLOUD_AUTHORIZE_URL
        );
        assert!(url.query().unwrap().contains("response_type=code"));
        assert!(url.query().unwrap().contains("code_challenge_method=S256"));
        assert!(url.query().unwrap().contains("state=state-123"));
    }

    #[test]
    fn parses_token_response() {
        let token: TokenResponse = serde_json::from_str(
            r#"{
                "access_token": "access",
                "refresh_token": "refresh",
                "expires_in": 3600,
                "scope": "non-expiring"
            }"#,
        )
        .unwrap();

        assert_eq!(token.access_token, "access");
        assert_eq!(token.refresh_token, "refresh");
    }
}
