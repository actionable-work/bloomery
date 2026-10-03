use bloomery_content::{OVERVIEW, render_markdown};
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{Unescaped, View, view};

use crate::components::{
    architecture_diagram_card, code_tabs_showcase, feature_cards, feedback_box, hero_banner,
    metrics_banner, next_steps_cards, page_layout,
};

#[page("/")]
pub async fn home() -> Result<impl View, Error> {
    Ok(view! {
        page_layout(
            title: "Fast, Hermetic Rust Builds with Nix",
            active_path: "/",
            <div style="display: flex; flex-direction: column; gap: 3rem;">
                <div class="doc-meta-bar">
                    <div class="breadcrumbs">
                        <a href="/" class="breadcrumb-link" style="display: flex; align-items: center; gap: 0.25rem;">
                            <span class="material-symbols-outlined" style="font-size: 15px;">"folder"</span>
                            <span>"Docs"</span>
                        </a>
                        <span class="breadcrumb-sep">"/"</span>
                        <span>"Getting Started"</span>
                        <span class="breadcrumb-sep">"/"</span>
                        <span class="breadcrumb-active">"Introduction"</span>
                    </div>
                    <div class="meta-badges">
                        <span class="meta-pill">
                            <span class="material-symbols-outlined" style="font-size: 14px;">"schedule"</span>
                            "5 min read"
                        </span>
                        <span class="meta-pill" style="color: var(--text-dim);">
                            <span class="status-dot"></span>
                            "v0.1.0"
                        </span>
                        <a href="https://github.com/actionable-work/bloomery" target="_blank" rel="noopener noreferrer" class="meta-edit-btn" title="View repository on GitHub">
                            <span class="material-symbols-outlined" style="font-size: 14px;">"code"</span>
                            "GitHub"
                        </a>
                    </div>
                </div>

                hero_banner()

                feature_cards()

                code_tabs_showcase()

                architecture_diagram_card()

                metrics_banner()

                <section style="display: flex; flex-direction: column; gap: 1.25rem;" id="philosophy">
                    <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                        <h2 class="doc-h2" style="margin: 0.25rem 0; border: none; padding: 0;">"The Bloomery Philosophy"</h2>
                        <p class="doc-paragraph" style="margin: 0; color: var(--text-muted);">
                            (OVERVIEW.description)
                        </p>
                    </div>
                    <div class="doc-body">
                        (Unescaped::new_unchecked(render_markdown(OVERVIEW.markdown)))
                    </div>
                </section>

                <section style="display: flex; flex-direction: column; gap: 1rem;" id="matrix">
                    <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                        <h2 class="doc-h2" style="margin: 0.25rem 0; border: none; padding: 0;">"Included Test Workspaces Matrix"</h2>
                        <p class="doc-paragraph" style="margin: 0; color: var(--text-muted);">
                            "Bloomery verifies its own correctness against 6 dedicated test workspaces, running 68+ automated checks in CI:"
                        </p>
                    </div>
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

                next_steps_cards()

                feedback_box()
            </div>
        )
    })
}
