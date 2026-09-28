use bloomery_content::{API, ARCHITECTURE, OVERRIDES, OVERVIEW, PROFILES, QUICKSTART};
use bloomery_ui::{
    default_bundle_dir, doc_page, doc_view, feature_cards, hero_banner, page_layout,
    prepare_asset_bundle,
};
use topcoat::Error;
use topcoat::asset::RouterBuilderAssetExt;
use topcoat::router::{Router, internal_serve, page};
use topcoat::view::{View, view};

#[page("/")]
async fn home() -> Result<impl View, Error> {
    Ok(view! {
        page_layout(
            title: "Pure Nix Rust Engine",
            active_path: "/",
            <div>
                hero_banner()

                feature_cards()

                doc_view(page: &OVERVIEW)

                <section style="margin-top: 3.5rem;">
                    <h2 class="doc-heading doc-h2">"Included Test Workspaces Matrix"</h2>
                    <p class="doc-paragraph">
                        "Bloomery verifies its own correctness against 6 dedicated test workspaces, running 68+ automated checks in CI:"
                    </p>
                    <div class="table-container">
                        <table class="data-table">
                            <thead>
                                <tr>
                                    <th>"Test Workspace"</th>
                                    <th>"Integration Style"</th>
                                    <th>"Description & Tested Levers"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td><code>"tests/basic-workspace"</code></td>
                                    <td><span class="pill emerald">"Style 1: Zero-Boilerplate"</span></td>
                                    <td>"Evaluates bloomery.mkFlake testing multi-crate workspace compilation, libs, binaries, and tests."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/axum-workspace"</code></td>
                                    <td><span class="pill cyan">"Style 2: Categorized mkWorkspace"</span></td>
                                    <td>"Full real-world Axum web server and Clap CLI with mold/lld linker settings and profiles."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/mklib-workspace"</code></td>
                                    <td><span class="pill forge">"Style 3: Custom Lib Constructor"</span></td>
                                    <td>"Constructor bloomery.mkLib pkgs with custom Nixpkgs instances, overlays, and system overrides."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/flake-parts-workspace"</code></td>
                                    <td><span class="pill">"Style 4: Flake-Parts Module"</span></td>
                                    <td>"Clean integration into flake-parts modules using bloomery.flakeModules.default."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/overrides-workspace"</code></td>
                                    <td><span class="pill forge">"Overrides Validation"</span></td>
                                    <td>"Validates colocated overrides.nix files (fileset unions, compilation flags, env) and override merging."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/edge-cases-workspace"</code></td>
                                    <td><span class="pill">"Edge Cases Validation"</span></td>
                                    <td>"Validates directory tests, multi-file binaries, weak feature syntax, and modern cargo:: directives."</td>
                                </tr>
                            </tbody>
                        </table>
                    </div>
                </section>

                <footer class="doc-footer">
                    <span class="doc-footer-brand">"Bloomery Documentation"</span>
                    <span class="doc-footer-meta">"Pure Nix Rust Engine • Zero-Cargo Builds"</span>
                </footer>
            </div>
        )
    })
}

#[page("/docs/quickstart")]
async fn quickstart() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &QUICKSTART, active_path: "/docs/quickstart")
    })
}

#[page("/profiles")]
async fn profiles() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &PROFILES, active_path: "/profiles")
    })
}

#[page("/docs/profiles")]
async fn docs_profiles() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &PROFILES, active_path: "/profiles")
    })
}

#[page("/api")]
async fn api() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &API, active_path: "/api")
    })
}

#[page("/docs/api")]
async fn docs_api() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &API, active_path: "/api")
    })
}

#[page("/overrides")]
async fn overrides() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &OVERRIDES, active_path: "/overrides")
    })
}

#[page("/docs/overrides")]
async fn docs_overrides() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &OVERRIDES, active_path: "/overrides")
    })
}

#[page("/docs/architecture")]
async fn architecture() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &ARCHITECTURE, active_path: "/docs/architecture")
    })
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let bundle_dir = default_bundle_dir();
    let bundle = prepare_asset_bundle(&bundle_dir).unwrap_or_else(|e| {
        panic!(
            "failed to prepare Topcoat asset bundle in {}: {e}",
            bundle_dir.display()
        )
    });

    println!("🔥 Starting Bloomery docs server on http://127.0.0.1:{port}");
    println!("📦 Assets served from {}", bundle.dir().display());

    let router = Router::builder()
        .assets(bundle)
        .page(home)
        .page(quickstart)
        .page(profiles)
        .page(docs_profiles)
        .page(api)
        .page(docs_api)
        .page(overrides)
        .page(docs_overrides)
        .page(architecture)
        .build();

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind port");

    internal_serve(listener, router.into(), std::future::pending())
        .await
        .expect("serve");
}
