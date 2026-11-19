//! Server wiring that serves the native Topcoat asset bundle.

use std::net::SocketAddr;
use topcoat::asset::{AssetBundle, RouterBuilderAssetExt};
use topcoat::router::{Router, internal_serve};

/// Builds the site router, attaching the bundle installed next to the binary.
pub fn router(bundle: Option<AssetBundle>) -> Router {
    let builder = Router::builder().page(site_ui::home);
    match bundle {
        Some(bundle) => builder.assets(bundle).build(),
        None => builder.build(),
    }
}

/// Serves the site on `PORT` (default 8080) until the process exits.
pub fn serve() {
    let port = std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(8080);
    let bundle = AssetBundle::load().expect("load the native asset bundle");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], port)))
            .await
            .expect("bind address");
        internal_serve(
            listener,
            router(Some(bundle)).into(),
            std::future::pending(),
        )
        .await
        .expect("serve");
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn router_builds_without_a_bundle() {
        let _ = super::router(None);
    }
}
