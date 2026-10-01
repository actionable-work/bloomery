use bloomery_ui::pages::register_pages;
use topcoat::asset::{AssetBundle, RouterBuilderAssetExt};
use topcoat::router::{Router, internal_serve};

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    println!("🔥 Starting Bloomery docs server on http://127.0.0.1:{port}");

    let mut builder = Router::builder();
    if let Ok(bundle) = AssetBundle::load() {
        println!(
            "📦 Loaded native Topcoat asset bundle from {}",
            bundle.dir().display()
        );
        builder = builder.assets(bundle);
    } else {
        eprintln!("⚠️ No Topcoat asset bundle found next to binary");
    }

    let router = register_pages(builder).build();

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind port");

    internal_serve(listener, router.into(), std::future::pending())
        .await
        .expect("serve");
}
