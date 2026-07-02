//! This crate contains all shared fullstack server functions.
use dioxus::prelude::*;

#[cfg(feature = "server")]
mod telemetry;
#[cfg(feature = "server")]
pub use telemetry::init;

/// Echo the user input on the server.
#[post("/api/echo")]
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    tracing::info!("echo called with input: {input:?}");
    Ok(input)
}
