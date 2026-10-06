//! Axum server library for the benchmark fixture.

use axum::{Router, routing::get};

/// Build a minimal router.
pub fn router() -> Router {
    Router::new().route("/", get(|| async { shared::label(leaf::answer()) }))
}
