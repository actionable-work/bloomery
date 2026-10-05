use bloomery_ui::pages::register_pages;
use std::net::SocketAddr;
use topcoat::asset::{AssetBundle, RouterBuilderAssetExt};
use topcoat::router::{Router, internal_serve};

fn configured_port(value: Option<&str>) -> u16 {
    value.and_then(|port| port.parse().ok()).unwrap_or(8080)
}

fn bind_address(port: u16) -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], port))
}

fn router_with_assets(bundle: Option<AssetBundle>) -> Router {
    let builder = Router::builder();
    let builder = match bundle {
        Some(bundle) => builder.assets(bundle),
        None => builder,
    };
    register_pages(builder).build()
}

async fn bind_listener(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    tokio::net::TcpListener::bind(bind_address(port)).await
}

#[tokio::main]
pub async fn run() {
    let port = configured_port(std::env::var("PORT").ok().as_deref());
    println!("🔥 Starting Bloomery docs server on http://127.0.0.1:{port}");

    let bundle = match AssetBundle::load() {
        Ok(bundle) => {
            println!(
                "📦 Loaded native Topcoat asset bundle from {}",
                bundle.dir().display()
            );
            Some(bundle)
        }
        Err(_) => {
            eprintln!("⚠️ No Topcoat asset bundle found next to binary");
            None
        }
    };
    let router = router_with_assets(bundle);

    let listener = bind_listener(port).await.expect("bind port");
    internal_serve(listener, router.into(), std::future::pending())
        .await
        .expect("serve");
}

#[cfg(test)]
mod tests {
    use super::{bind_address, bind_listener, configured_port, internal_serve, router_with_assets};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::Ipv4Addr;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use topcoat::asset::{AssetBundle, Manifest};
    use topcoat::router::{Body, HeaderMap, Router, request::Request, to_bytes};

    static BUNDLE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_bundle() -> (AssetBundle, PathBuf) {
        let suffix = BUNDLE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "bloomery-docs-assets-{}-{suffix}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("asset directory");

        let assets = [
            (
                bloomery_ui::assets::BLOOMERY_CSS.id(),
                "bloomery.css",
                "text/css",
                "body { color: white; }",
            ),
            (
                bloomery_ui::assets::BLOOMERY_LOGO.id(),
                "bloomery-logo.svg",
                "image/svg+xml",
                "<svg></svg>",
            ),
            (
                bloomery_ui::assets::BLOOMERY_FAVICON.id(),
                "favicon.svg",
                "image/svg+xml",
                "<svg></svg>",
            ),
        ];
        let mut manifest_text = String::from("version = 1\n");
        for (id, filename, content_type, contents) in assets {
            fs::write(directory.join(filename), contents).expect("asset file");
            manifest_text.push_str(&format!(
                "\n[[assets]]\nid = {}\nfile = \"{}\"\nhash = \"{}\"\ncontent_type = \"{}\"\n",
                id.as_u64(),
                filename,
                "0".repeat(64),
                content_type
            ));
        }
        let manifest = Manifest::parse(&manifest_text).expect("asset manifest");
        manifest
            .save(directory.join("manifest.toml"))
            .expect("write manifest");
        let bundle = AssetBundle::load_dir(&directory).expect("load asset bundle");
        (bundle, directory)
    }

    fn get(router: &Router, path: &str) -> (u16, HeaderMap, Vec<u8>) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("Tokio runtime");
        let response = runtime.block_on(async {
            let request = Request::builder()
                .uri(path)
                .body(Body::empty())
                .expect("HTTP request");
            router.handle(request).await
        });
        let (parts, body) = response.into_parts();
        let bytes = runtime
            .block_on(to_bytes(body, usize::MAX))
            .expect("response body");
        (parts.status.as_u16(), parts.headers, bytes.to_vec())
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-SERVER-STARTUP-003"))]
    fn valid_port_environment_values_select_the_requested_port() {
        assert_eq!(configured_port(Some("43127")), 43127);
        assert_eq!(bind_address(configured_port(Some("43127"))).port(), 43127);
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-SERVER-STARTUP-004"))]
    fn missing_or_invalid_port_values_use_the_default() {
        assert_eq!(configured_port(None), 8080);
        assert_eq!(configured_port(Some("not-a-port")), 8080);
        assert_eq!(configured_port(Some("65536")), 8080);
        assert_eq!(configured_port(Some("")), 8080);
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-SERVER-STARTUP-002"))]
    fn listener_binds_to_ipv4_localhost() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .expect("Tokio runtime");
        let listener = runtime
            .block_on(bind_listener(0))
            .expect("localhost listener");
        let address = listener.local_addr().expect("listener address");

        assert_eq!(address.ip(), Ipv4Addr::LOCALHOST);
        assert_ne!(address.port(), 0);
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-SERVER-STARTUP-001"))]
    fn server_serves_registered_ui_pages_over_local_http() {
        let (bundle, directory) = test_bundle();
        let router = router_with_assets(Some(bundle));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Tokio runtime");
        let response = runtime.block_on(async {
            let listener = bind_listener(0).await.expect("listener");
            let address = listener.local_addr().expect("listener address");
            let server = tokio::spawn(internal_serve(
                listener,
                router.into(),
                std::future::pending(),
            ));
            let request = tokio::task::spawn_blocking(move || -> std::io::Result<String> {
                let mut stream = std::net::TcpStream::connect(address)?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                stream.write_all(
                    b"GET /docs/overview HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
                )?;
                let mut response = String::new();
                stream.read_to_string(&mut response)?;
                Ok(response)
            });
            let response = request.await.expect("HTTP client task");
            server.abort();
            let _ = server.await;
            response.expect("HTTP response")
        });

        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("Overview &amp; Philosophy"));
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-SERVER-STARTUP-005"))]
    fn an_available_native_asset_bundle_is_attached_to_the_router() {
        let (bundle, directory) = test_bundle();
        let router = router_with_assets(Some(bundle));
        let (status, headers, body) = get(&router, "/_topcoat/assets/bloomery.css");

        assert_eq!(status, 200);
        assert_eq!(
            headers
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("text/css")
        );
        assert_eq!(body, b"body { color: white; }");
        let _ = fs::remove_dir_all(directory);
    }
}
