#![cfg(feature = "server")]

//! Basic HTTP authentication middleware.
//!
//! This middleware is only enabled when the `BASIC_AUTH_USERNAME` and
//! `BASIC_AUTH_PASSWORD` environment variables are both set at runtime. It is
//! intended for protecting non-production environments (e.g., the dev Render
//! service) while leaving local development and production builds unaffected.

use base64::prelude::{BASE64_STANDARD, Engine as _};
use http::{Request, Response, StatusCode, header};
use subtle::ConstantTimeEq;
use tower_http::auth::AsyncAuthorizeRequest;

#[derive(Clone)]
pub struct BasicAuth {
    username: String,
    password: String,
}

impl BasicAuth {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }
}

impl<B> AsyncAuthorizeRequest<B> for BasicAuth
where
    B: 'static,
{
    type RequestBody = B;
    type ResponseBody = dioxus::server::axum::body::Body;
    type Future = std::future::Ready<Result<Request<B>, Response<Self::ResponseBody>>>;

    fn authorize(&mut self, request: Request<B>) -> Self::Future {
        let expected = format!("{}:{}", self.username, self.password);

        let is_valid = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Basic "))
            .and_then(|encoded| BASE64_STANDARD.decode(encoded).ok())
            .and_then(|bytes| String::from_utf8(bytes).ok())
            .map(|credentials| credentials.as_bytes().ct_eq(expected.as_bytes()).into())
            .unwrap_or(false);

        if is_valid {
            std::future::ready(Ok(request))
        } else {
            let response = Response::builder()
                .status(StatusCode::UNAUTHORIZED)
                .header(header::WWW_AUTHENTICATE, "Basic realm=\"dev\"")
                .body(Self::ResponseBody::default())
                .expect("failed to build unauthorized response");
            std::future::ready(Err(response))
        }
    }
}
