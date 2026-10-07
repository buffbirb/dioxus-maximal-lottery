//! GitHub sign-in, through a GitHub App's user-authorization flow.
//!
//! - No scopes: the public profile needs none, and GitHub Apps ignore them.
//! - The access token lives only inside `exchange`; it is never stored.
//! - The subject is the numeric id, since logins can be renamed and reused.
use std::borrow::Cow;
use std::sync::OnceLock;
use std::time::Duration;

use oauth2::basic::BasicClient;
use oauth2::url::Url;
use oauth2::{
    AccessToken, AsyncHttpClient, AuthType, AuthUrl, AuthorizationCode, ClientId, ClientSecret,
    CsrfToken, EndpointNotSet, EndpointSet, PkceCodeChallenge, PkceCodeVerifier, RedirectUrl,
    TokenResponse, TokenUrl,
};
use serde::Deserialize;

use super::provider::{AuthError, Callback, Identity};

const ID: &str = "github";
const CLIENT_ID_VAR: &str = "OAUTH_GITHUB_CLIENT_ID";
const CLIENT_SECRET_VAR: &str = "OAUTH_GITHUB_CLIENT_SECRET";

const AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";
const API_VERSION: &str = "2022-11-28";
/// GitHub rejects API requests without a User-Agent.
const USER_AGENT: &str = "maximal-lottery";
const TIMEOUT: Duration = Duration::from_secs(10);

type Client = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

struct Config {
    oauth: Client,
    http: reqwest::Client,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init() {
    let config = Config {
        oauth: oauth_client(required(CLIENT_ID_VAR), required(CLIENT_SECRET_VAR)),
        http: reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            // oauth2 advises against following redirects (SSRF).
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to build the GitHub HTTP client"),
    };
    if CONFIG.set(config).is_err() {
        panic!("auth::init must only be called once");
    }
}

fn required(name: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| panic!("{name} must be set and non-empty"))
}

fn config() -> &'static Config {
    CONFIG
        .get()
        .expect("GitHub sign-in not initialized; call auth::init() first")
}

fn oauth_client(client_id: String, client_secret: String) -> Client {
    BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new(AUTH_URL.to_string()).expect("valid GitHub authorize URL"))
        .set_token_uri(TokenUrl::new(TOKEN_URL.to_string()).expect("valid GitHub token URL"))
        .set_auth_type(AuthType::RequestBody)
}

pub fn authorize_url(
    redirect_uri: &RedirectUrl,
    state: CsrfToken,
    code_challenge: PkceCodeChallenge,
) -> Url {
    authorize_url_with(&config().oauth, redirect_uri, state, code_challenge)
}

fn authorize_url_with(
    client: &Client,
    redirect_uri: &RedirectUrl,
    state: CsrfToken,
    code_challenge: PkceCodeChallenge,
) -> Url {
    client
        .authorize_url(|| state)
        .set_pkce_challenge(code_challenge)
        .set_redirect_uri(Cow::Borrowed(redirect_uri))
        .url()
        .0
}

pub async fn exchange(
    callback: &Callback,
    redirect_uri: &RedirectUrl,
) -> Result<Identity, AuthError> {
    let config = config();
    let token = request_token(&config.oauth, &config.http, callback, redirect_uri).await?;
    let profile = fetch_profile(&config.http, &token).await?;
    Ok(identity_from_profile(profile))
}

async fn request_token<'c, C>(
    client: &'c Client,
    http: &'c C,
    callback: &Callback,
    redirect_uri: &RedirectUrl,
) -> Result<AccessToken, AuthError>
where
    C: AsyncHttpClient<'c>,
{
    client
        .exchange_code(AuthorizationCode::new(callback.code.clone()))
        .set_pkce_verifier(PkceCodeVerifier::new(callback.code_verifier.clone()))
        .set_redirect_uri(Cow::Owned(redirect_uri.clone()))
        .request_async(http)
        .await
        .map(|response| response.access_token().clone())
        .map_err(|e| AuthError::Upstream(format!("token exchange: {e}")))
}

async fn fetch_profile(http: &reqwest::Client, token: &AccessToken) -> Result<Profile, AuthError> {
    let upstream = |e: reqwest::Error| AuthError::Upstream(format!("profile fetch: {e}"));
    http.get(USER_URL)
        .bearer_auth(token.secret())
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .send()
        .await
        .map_err(upstream)?
        .error_for_status()
        .map_err(upstream)?
        .json()
        .await
        .map_err(upstream)
}

#[derive(Debug, Deserialize)]
struct Profile {
    id: u64,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
}

/// `name` is optional on GitHub, so `login` stands in and a display name is
/// always present.
fn identity_from_profile(profile: Profile) -> Identity {
    let name = profile
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string);
    Identity {
        provider: ID,
        subject: profile.id.to_string(),
        display_name: Some(name.unwrap_or(profile.login)),
        avatar_url: profile.avatar_url.filter(|url| !url.is_empty()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use oauth2::HttpRequest;

    use super::*;

    const REDIRECT: &str = "https://polls.example.com/login/github/callback";

    fn client() -> Client {
        oauth_client("cid".to_string(), "csecret".to_string())
    }

    fn redirect_uri() -> RedirectUrl {
        RedirectUrl::new(REDIRECT.to_string()).expect("valid redirect")
    }

    fn query(url: &Url) -> HashMap<String, String> {
        url.query_pairs().into_owned().collect()
    }

    #[test]
    fn authorize_url_carries_exactly_the_pkce_flow_parameters() {
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let expected_challenge = PkceCodeChallenge::from_code_verifier_sha256(&verifier);
        let url = authorize_url_with(
            &client(),
            &redirect_uri(),
            CsrfToken::new("st".to_string()),
            challenge,
        );

        assert_eq!(
            url.as_str().split('?').next(),
            Some("https://github.com/login/oauth/authorize")
        );
        let params = query(&url);
        let mut keys: Vec<_> = params.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "client_id",
                "code_challenge",
                "code_challenge_method",
                "redirect_uri",
                "response_type",
                "state",
            ]
        );
        assert_eq!(params["client_id"], "cid");
        assert_eq!(params["redirect_uri"], REDIRECT);
        assert_eq!(params["state"], "st");
        assert_eq!(params["code_challenge"], expected_challenge.as_str());
        assert_eq!(params["code_challenge_method"], "S256");
        assert!(!params.contains_key("scope"));
        assert!(!url.as_str().contains(verifier.secret()));
    }

    #[tokio::test]
    async fn token_request_carries_the_verifier_and_redirect_uri() {
        let captured: Mutex<Option<HttpRequest>> = Mutex::new(None);
        let fake = |request: HttpRequest| {
            *captured.lock().unwrap() = Some(request);
            async {
                Ok::<_, std::io::Error>(
                    http::Response::builder()
                        .status(200)
                        .header("content-type", "application/json")
                        .body(br#"{"access_token":"tok","token_type":"bearer"}"#.to_vec())
                        .expect("fake response"),
                )
            }
        };
        let verifier = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ0123-_6";
        let callback = Callback {
            code: "the-code".to_string(),
            code_verifier: verifier.to_string(),
            params: HashMap::new(),
        };

        let token = request_token(&client(), &fake, &callback, &redirect_uri())
            .await
            .expect("token");
        assert_eq!(token.secret(), "tok");

        let request = captured.into_inner().unwrap().expect("a request was sent");
        assert_eq!(request.uri().to_string(), TOKEN_URL);
        let body: HashMap<String, String> = oauth2::url::form_urlencoded::parse(request.body())
            .into_owned()
            .collect();
        assert_eq!(body["grant_type"], "authorization_code");
        assert_eq!(body["code"], "the-code");
        assert_eq!(body["code_verifier"], verifier);
        assert_eq!(body["redirect_uri"], REDIRECT);
        assert_eq!(body["client_id"], "cid");
        assert_eq!(body["client_secret"], "csecret");
    }

    #[test]
    fn display_name_falls_back_to_login() {
        let profile: Profile =
            serde_json::from_str(r#"{ "id": 42, "login": "octocat", "name": "  " }"#).unwrap();
        assert_eq!(
            identity_from_profile(profile),
            Identity {
                provider: "github",
                subject: "42".to_string(),
                display_name: Some("octocat".to_string()),
                avatar_url: None,
            }
        );
    }

    #[test]
    fn full_profile_maps_every_field() {
        let profile: Profile = serde_json::from_str(
            r#"{ "id": 7, "login": "mona", "name": " Mona Lisa ",
                 "avatar_url": "https://avatars.githubusercontent.com/u/7" }"#,
        )
        .unwrap();
        let identity = identity_from_profile(profile);
        assert_eq!(identity.subject, "7");
        assert_eq!(identity.display_name.as_deref(), Some("Mona Lisa"));
        assert_eq!(
            identity.avatar_url.as_deref(),
            Some("https://avatars.githubusercontent.com/u/7")
        );
    }

    #[test]
    fn null_name_and_avatar_are_absent() {
        let profile: Profile =
            serde_json::from_str(r#"{ "id": 1, "login": "x", "name": null, "avatar_url": null }"#)
                .unwrap();
        let identity = identity_from_profile(profile);
        assert_eq!(identity.display_name.as_deref(), Some("x"));
        assert_eq!(identity.avatar_url, None);
    }
}
