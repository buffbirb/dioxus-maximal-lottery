pub mod model;

#[cfg(feature = "server")]
mod db;
#[cfg(feature = "server")]
mod lottery;
#[cfg(feature = "server")]
mod telemetry;

pub use model::*;

#[cfg(feature = "server")]
pub use db::{DbPool, get_pool, init_pool, run_migrations};
#[cfg(feature = "server")]
pub use telemetry::init;

mod polls;
pub use polls::*;

use dioxus::prelude::*;

#[server]
pub async fn echo(input: String) -> Result<String, ServerFnError> {
    Ok(input)
}
