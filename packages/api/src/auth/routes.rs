//! The OAuth routes. Plain axum handlers, not server functions: they redirect
//! and set several cookies per response, and a server function gets one
//! header slot per name.
//!
//! Codes, states, and tokens are never logged.
use std::collections::HashMap;

use chrono::Utc;
use dioxus::server::axum::{
    Router,
    extract::{Path, Query},
    response::{AppendHeaders, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use http::{StatusCode, header::SET_COOKIE, request::Parts};
use oauth2::{CsrfToken, PkceCodeChallenge, RedirectUrl};

use super::provider::{Callback, Provider};
use super::{return_to, session, state};
use crate::{cookies, db, origin};

pub fn router() -> Router {
    Router::new()
        .route("/login/{provider}", get(start))
        .route("/login/{provider}/callback", get(callback))
        .route("/logout", post(logout))
}

/// Recomputed identically by the callback; the provider rejects any value
/// not registered for the app, so a spoofed Host cannot redirect the code.
fn redirect_uri(parts: &Parts, provider: Provider) -> Option<RedirectUrl> {
    let origin = origin::derive_origin(parts)?;
    RedirectUrl::new(format!("{origin}/login/{}/callback", provider.id())).ok()
}

async fn start(
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    parts: Parts,
) -> Response {
    let Some(provider) = Provider::from_id(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(redirect_uri) = redirect_uri(&parts, provider) else {
        tracing::error!("no trustworthy request origin for the redirect URI; set PUBLIC_BASE_URL");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    let return_to = return_to::sanitize(query.get("return_to").map(String::as_str));
    let state = CsrfToken::new_random();
    let (code_challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let secure = cookies::is_https_request(&parts);
    let cookie = state::set_header(&state, &verifier, &return_to, secure);
    let url = provider.authorize_url(&redirect_uri, state, code_challenge);

    (
        AppendHeaders([(SET_COOKIE, cookie)]),
        Redirect::to(url.as_str()),
    )
        .into_response()
}

async fn callback(
    Path(id): Path<String>,
    Query(mut params): Query<HashMap<String, String>>,
    parts: Parts,
) -> Response {
    let Some(provider) = Provider::from_id(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let secure = cookies::is_https_request(&parts);
    // Single-use: cleared on every outcome below.
    let clear_state = (SET_COOKIE, state::clear_header(secure));
    let fail = |status: StatusCode| (status, AppendHeaders([clear_state.clone()])).into_response();

    let stored = cookies::value(&parts, state::NAME).and_then(|value| state::parse(&value));
    let presented = params.remove("state");
    let (Some(stored), Some(presented)) = (stored, presented) else {
        return fail(StatusCode::BAD_REQUEST);
    };
    if !state::matches(&stored.state, &presented) {
        return fail(StatusCode::BAD_REQUEST);
    }

    if let Some(error) = params.get("error") {
        tracing::info!(
            provider = provider.id(),
            error,
            "sign-in not completed at the provider"
        );
        // Back to the chooser, still bound for where the user started.
        return (
            AppendHeaders([clear_state]),
            Redirect::to(&format!("/login?return_to={}", stored.return_to)),
        )
            .into_response();
    }

    let Some(code) = params.remove("code") else {
        return fail(StatusCode::BAD_REQUEST);
    };
    let Some(redirect_uri) = redirect_uri(&parts, provider) else {
        tracing::error!("no trustworthy request origin for the redirect URI; set PUBLIC_BASE_URL");
        return fail(StatusCode::INTERNAL_SERVER_ERROR);
    };

    let callback = Callback {
        code,
        code_verifier: stored.code_verifier,
        params,
    };
    let identity = match provider.exchange(&callback, &redirect_uri).await {
        Ok(identity) => identity,
        Err(e) => {
            tracing::warn!(provider = provider.id(), "sign-in failed: {e}");
            return fail(StatusCode::BAD_GATEWAY);
        }
    };

    let fallback_name = format!("{} user", provider.label());
    let user_id = match db::upsert_identity(
        identity.provider,
        &identity.subject,
        identity.display_name.as_deref(),
        identity.avatar_url.as_deref(),
        &fallback_name,
    )
    .await
    {
        Ok(user_id) => user_id,
        Err(e) => {
            tracing::error!(provider = provider.id(), "account lookup failed: {e}");
            return fail(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // The cookie below replaces any session the browser already holds; revoke
    // that row too, or it stays valid until expiry.
    if let Some(previous) = session::token_from_request(&parts)
        && let Err(e) = db::delete_session(&cookies::hash_token(&previous)).await
    {
        tracing::warn!("previous session delete failed: {e}");
    }

    let token = cookies::new_token();
    let expires_at = Utc::now() + chrono::Duration::seconds(session::MAX_AGE_SECS);
    if let Err(e) = db::insert_session(user_id, &cookies::hash_token(&token), expires_at).await {
        tracing::error!("session insert failed: {e}");
        return fail(StatusCode::INTERNAL_SERVER_ERROR);
    }

    (
        AppendHeaders([
            clear_state,
            (SET_COOKIE, session::set_header(&token, secure)),
        ]),
        Redirect::to(&stored.return_to),
    )
        .into_response()
}

/// Clears the cookie even when no row matches or the delete fails.
async fn logout(parts: Parts) -> Response {
    if let Some(token) = session::token_from_request(&parts)
        && let Err(e) = db::delete_session(&cookies::hash_token(&token)).await
    {
        tracing::warn!("session delete failed: {e}");
    }
    let secure = cookies::is_https_request(&parts);
    (
        AppendHeaders([(SET_COOKIE, session::clear_header(secure))]),
        Redirect::to("/"),
    )
        .into_response()
}
