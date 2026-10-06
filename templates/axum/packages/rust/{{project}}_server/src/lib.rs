//! Axum router and handlers.

use axum::{Json, Router, response::Html, routing::get};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

const INDEX_HTML: &str = include_str!("../assets/index.html");

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Health {
    pub status: String,
}

async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn health() -> Json<Health> {
    Json(Health {
        status: "ok".to_string(),
    })
}

/// Builds the application router.
pub fn app() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/health", get(health))
}

/// Runs the HTTP server until the process exits.
pub fn serve() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(async {
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("bind address");
        axum::serve(listener, app()).await.expect("serve");
    });
}

#[cfg(test)]
mod tests {
    use super::{Health, INDEX_HTML};

    #[cfg_attr(any(), bloomery("SERVER-HTTP-ROUTES-001"))]
    #[test]
    fn index_asset_is_embedded() {
        assert!(INDEX_HTML.contains("<!doctype html>"));
    }

    #[cfg_attr(any(), bloomery("SERVER-HTTP-ROUTES-002"))]
    #[test]
    fn health_reports_ok() {
        let health = Health {
            status: "ok".to_string(),
        };
        assert_eq!(health.status, "ok");
    }
}
