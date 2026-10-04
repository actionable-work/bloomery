use topcoat::{
    Result,
    view::{Unescaped, View, component, view},
};

#[component]
pub async fn sidebar(active_path: &str) -> Result<impl View> {
    Ok(view! {
        <aside class="bloomery-sidebar">
            <nav class="sidebar-nav">
                <div class="sidebar-group">
                    <div class="sidebar-group-header">
                        <span class="sidebar-group-title">
                            <span class="material-symbols-outlined" style="font-size: 15px;">"terminal"</span>
                            "Getting Started"
                        </span>
                        <span class="material-symbols-outlined" style="font-size: 14px; opacity: 0.5;">"expand_more"</span>
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.2rem;">
                        <a href="/" class=(if active_path == "/" || active_path == "/overview" || active_path == "/docs/overview" { "sidebar-link active" } else { "sidebar-link" })>
                            "Introduction"
                        </a>
                        <a href="/docs/quickstart" class=(if active_path == "/quickstart" || active_path == "/docs/quickstart" { "sidebar-link active" } else { "sidebar-link" })>
                            "Quickstart"
                        </a>
                        <a href="/docs/architecture" class=(if active_path == "/architecture" || active_path == "/docs/architecture" { "sidebar-link active" } else { "sidebar-link" })>
                            "Architecture"
                        </a>
                    </div>
                </div>

                <div class="sidebar-group">
                    <div class="sidebar-group-header">
                        <span class="sidebar-group-title">
                            <span class="material-symbols-outlined" style="font-size: 15px;">"settings_input_component"</span>
                            "Configuration"
                        </span>
                        <span class="material-symbols-outlined" style="font-size: 14px; opacity: 0.5;">"expand_more"</span>
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.2rem;">
                        <a href="/profiles" class=(if active_path == "/profiles" || active_path == "/docs/profiles" { "sidebar-link active" } else { "sidebar-link" })>
                            "Strongly-Typed Profiles"
                        </a>
                        <a href="/api" class=(if active_path == "/api" || active_path == "/docs/api" { "sidebar-link active" } else { "sidebar-link" })>
                            "mkFlake Config API"
                        </a>
                        <a href="/overrides" class=(if active_path == "/overrides" || active_path == "/docs/overrides" { "sidebar-link active" } else { "sidebar-link" })>
                            "Colocated overrides.nix"
                        </a>
                    </div>
                </div>

                <div class="sidebar-group">
                    <div class="sidebar-group-header">
                        <span class="sidebar-group-title">
                            <span class="material-symbols-outlined" style="font-size: 15px;">"analytics"</span>
                            "Validation & Benchmarks"
                        </span>
                        <span class="material-symbols-outlined" style="font-size: 14px; opacity: 0.5;">"expand_more"</span>
                    </div>
                    <div style="display: flex; flex-direction: column; gap: 0.2rem;">
                        <a href="/matrix" class=(if active_path == "/matrix" || active_path == "/docs/matrix" { "sidebar-link active" } else { "sidebar-link" })>
                            "Verification Matrix"
                        </a>
                        <a href="/benchmarks" class=(if active_path == "/benchmarks" || active_path == "/docs/benchmarks" { "sidebar-link active" } else { "sidebar-link" })>
                            "Architectural Benchmarks"
                        </a>
                    </div>
                </div>
            </nav>
        </aside>
    })
}

#[component]
pub async fn toc_sidebar(active_path: &str) -> Result<impl View> {
    let clean_path = active_path.trim_end_matches('/');
    let path = if clean_path.is_empty() {
        "/"
    } else {
        clean_path
    };

    let items: &[(&str, &str, bool)] = match path {
        "/quickstart" | "/docs/quickstart" => &[
            (
                "#zero-boilerplate-with-mkflake",
                "Zero-Boilerplate mkFlake",
                false,
            ),
            (
                "#alternative-integration-styles",
                "Integration Styles",
                false,
            ),
            (
                "#generating-the-lock-manifest",
                "Generating Lockfile",
                false,
            ),
            ("#running-and-checking", "Running & Checking", false),
        ],
        "/profiles" | "/docs/profiles" => &[
            ("#profile-options-matrix", "Profile Options Matrix", false),
            (
                "#configuring-release-and-dev-profiles",
                "Configuring Profiles",
                false,
            ),
        ],
        "/api" | "/docs/api" => &[
            ("#mkflake-constructor", "mkFlake Constructor", false),
            ("#configuration-tables", "Configuration Tables", false),
            ("#overrides-argument", "overrides Argument", false),
            ("#workspace-return-value", "Workspace Return Value", false),
        ],
        "/overrides" | "/docs/overrides" => &[
            (
                "#anatomy-of-an-overrides-nix",
                "Anatomy of overrides.nix",
                false,
            ),
            (
                "#native-framework-asset-pipeline",
                "Native Asset Pipeline",
                false,
            ),
            (
                "#override-precedence-merging",
                "Precedence & Merging",
                false,
            ),
        ],
        "/architecture" | "/docs/architecture" => &[
            ("#zero-ifd-feature-resolution", "Zero-IFD Resolution", false),
            ("#per-crate-derivation-graph", "Per-Crate DAG", false),
            (
                "#modern-build-script-directives",
                "Build Script Sandboxing",
                false,
            ),
            (
                "#native-framework-asset-aggregation",
                "Native Asset Aggregation",
                false,
            ),
        ],
        "/matrix" | "/docs/matrix" => {
            &[("#workspace-integration-matrix", "Integration Matrix", false)]
        }
        "/benchmarks" | "/docs/benchmarks" => &[
            ("#performance-comparison", "Performance", false),
            (
                "#architectural-comparison-matrix",
                "Comparison Matrix",
                false,
            ),
        ],
        "/docs/overview" | "/overview" => &[
            ("#why-pure-nix-derivations", "Why Pure Nix?", false),
            ("#core-capabilities", "Core Capabilities", false),
        ],
        _ => &[
            ("#why-pure-nix-derivations", "Why Pure Nix?", false),
            ("#core-capabilities", "Core Capabilities", false),
        ],
    };

    let mut toc_html = String::new();
    for (href, label, indent) in items {
        let class = if *indent {
            "toc-link indent"
        } else {
            "toc-link"
        };
        toc_html.push_str(&format!(
            "<a href=\"{href}\" class=\"{class}\">{label}</a>\n"
        ));
    }

    Ok(view! {
        <aside class="bloomery-toc-sidebar">
            <div class="toc-group">
                <div class="toc-title">"On this page"</div>
                <div class="toc-list">
                    (Unescaped::new_unchecked(toc_html))
                </div>
            </div>

            <div class="community-group">
                <div class="toc-title">"Community & Links"</div>
                <div class="toc-list">
                    <a href="https://github.com/actionable-work/bloomery" target="_blank" rel="noopener noreferrer" class="community-link">
                        <span class="material-symbols-outlined" style="font-size: 16px;">"code"</span>
                        <span>"GitHub Repository"</span>
                    </a>
                    <a href="https://github.com/actionable-work/bloomery/issues/new" target="_blank" rel="noopener noreferrer" class="community-link">
                        <span class="material-symbols-outlined" style="font-size: 16px;">"bug_report"</span>
                        <span>"Report docs issue"</span>
                    </a>
                    <a href="https://github.com/actionable-work/bloomery/discussions" target="_blank" rel="noopener noreferrer" class="community-link">
                        <span class="material-symbols-outlined" style="font-size: 16px;">"forum"</span>
                        <span>"Ask on Discussions"</span>
                    </a>
                </div>
            </div>
        </aside>
    })
}
