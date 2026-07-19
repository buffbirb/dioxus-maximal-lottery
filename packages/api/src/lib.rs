//! This crate contains all shared fullstack server functions.
use dioxus::prelude::*;

pub mod domain;
pub mod model;
pub mod polls;

#[cfg(feature = "server")]
pub mod db;
#[cfg(feature = "server")]
pub mod lottery;

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
