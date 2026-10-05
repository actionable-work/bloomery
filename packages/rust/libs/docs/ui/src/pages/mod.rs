pub mod api;
pub mod architecture;
pub mod benchmarks;
pub mod home;
pub mod matrix;
pub mod overrides;
pub mod overview;
pub mod profiles;
pub mod quickstart;

pub use api::*;
pub use architecture::*;
pub use benchmarks::*;
pub use home::*;
pub use matrix::*;
pub use overrides::*;
pub use overview::*;
pub use profiles::*;
pub use quickstart::*;

use topcoat::router::RouterBuilder;

/// Register all documentation and application pages onto a Topcoat router builder.
pub fn register_pages(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(home)
        .page(overview)
        .page(docs_overview)
        .page(quickstart)
        .page(docs_quickstart)
        .page(architecture)
        .page(docs_architecture)
        .page(profiles)
        .page(docs_profiles)
        .page(api)
        .page(docs_api)
        .page(overrides)
        .page(docs_overrides)
        .page(matrix)
        .page(docs_matrix)
        .page(benchmarks)
        .page(docs_benchmarks)
}

#[cfg(test)]
mod tests {
    use super::register_pages;
    use crate::assets::{BLOOMERY_CSS, BLOOMERY_FAVICON, BLOOMERY_LOGO};
    use bloomery_content::all_pages;
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use topcoat::asset::{AssetConfig, Manifest, RouterBuilderAssetExt};
    use topcoat::router::{Body, Router, request::Request, to_bytes};

    struct ThreadWake(std::thread::Thread);

    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    fn router() -> Router {
        let mut manifest = String::from("version = 1\n");
        for (asset, name, content_type) in [
            (BLOOMERY_CSS, "bloomery.css", "text/css"),
            (BLOOMERY_LOGO, "bloomery-logo.svg", "image/svg+xml"),
            (BLOOMERY_FAVICON, "favicon.svg", "image/svg+xml"),
        ] {
            manifest.push_str(&format!(
                "\n[[assets]]\nid = {}\nfile = \"{}\"\nhash = \"{}\"\ncontent_type = \"{}\"\n",
                asset.id().as_u64(),
                name,
                "0".repeat(64),
                content_type
            ));
        }
        let manifest = Manifest::parse(&manifest).expect("test asset manifest");
        let assets = AssetConfig::hosted_at("https://assets.example.test", manifest);
        register_pages(Router::builder().assets(assets)).build()
    }

    fn get(router: &Router, path: &str) -> (u16, String) {
        let request = Request::builder()
            .uri(path)
            .body(Body::empty())
            .expect("HTTP request");
        let response = block_on(router.handle(request));
        let status = response.status().as_u16();
        let body = block_on(to_bytes(response.into_body(), usize::MAX)).expect("response body");
        (
            status,
            String::from_utf8(body.to_vec()).expect("HTML response"),
        )
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-UI-ROUTES-001"))]
    fn registers_and_serves_the_home_page() {
        let (status, html) = get(&router(), "/");
        assert_eq!(status, 200);
        assert!(html.contains("Fast, Hermetic Rust Builds with Nix"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-UI-ROUTES-002"))]
    fn exposes_every_catalog_page_at_its_short_route() {
        let router = router();
        for page in all_pages() {
            let (status, html) = get(&router, &format!("/{}", page.slug));
            assert_eq!(status, 200, "short route for {}", page.slug);
            let title = page.title.replace('&', "&amp;");
            assert!(html.contains(&title), "title missing at /{}", page.slug);
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-UI-ROUTES-003"))]
    fn exposes_every_catalog_page_at_its_docs_route() {
        let router = router();
        for page in all_pages() {
            let (status, html) = get(&router, &format!("/docs/{}", page.slug));
            assert_eq!(status, 200, "docs route for {}", page.slug);
            let title = page.title.replace('&', "&amp;");
            assert!(
                html.contains(&title),
                "title missing at /docs/{}",
                page.slug
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-UI-ROUTES-004"))]
    fn documentation_pages_use_the_shared_site_layout() {
        let (status, html) = get(&router(), "/docs/overview");
        assert_eq!(status, 200);
        assert!(html.contains("class=\"bloomery-shell\""));
        assert!(html.contains("class=\"bloomery-top-header\""));
        assert!(html.contains("class=\"bloomery-content\""));
        assert!(html.contains("https://assets.example.test/bloomery.css"));
    }
}
