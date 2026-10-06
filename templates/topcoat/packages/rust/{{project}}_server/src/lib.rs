//! Server wiring for the website.

use app_ui::home;
use std::net::SocketAddr;
use topcoat::router::{Router, internal_serve};

/// Builds the site router.
pub fn router() -> Router {
    Router::builder().page(home).build()
}

/// Serves the website until the process exits.
pub fn serve() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(async {
        let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .expect("bind address");
        internal_serve(listener, router().into(), std::future::pending())
            .await
            .expect("serve");
    });
}

#[cfg(test)]
mod tests {
    #[cfg_attr(any(), bloomery("SITE-SERVER-SERVING-001"))]
    #[test]
    fn router_builds() {
        let _ = super::router();
    }
}
