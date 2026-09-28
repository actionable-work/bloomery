use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn feature_cards() -> Result<impl View> {
    Ok(view! {
        <div class="grid-cards">
            <div class="feature-card">
                <div class="feature-icon">"📦"</div>
                <div class="feature-title">"True Per-Crate Derivations"</div>
                <div class="feature-body">
                    "Every crate in Cargo.lock compiles as an isolated .rlib derivation. Modifying a file rebuilds only that crate; dependencies stay 100% cached."
                </div>
            </div>

            <div class="feature-card">
                <div class="feature-icon">"⚡"</div>
                <div class="feature-title">"Parallel Check Suites"</div>
                <div class="feature-body">
                    "Run tests, clippy-driver, rustdoc, and doctest checks in parallel derivations without Cargo lock contention or redundant compilation."
                </div>
            </div>

            <div class="feature-card">
                <div class="feature-icon">"🔒"</div>
                <div class="feature-title">"Zero-IFD Lock Manifest"</div>
                <div class="feature-body">
                    "Lock resolution pre-computes activated features and dependencies into bloomery.lock with zero evaluation overhead or IFD pauses."
                </div>
            </div>

            <div class="feature-card">
                <div class="feature-icon">"⚙️"</div>
                <div class="feature-title">"Strongly-Typed Profiles"</div>
                <div class="feature-body">
                    "Evaluate LTO, codegen-units, panic strategies, and CPU targets with validated Nix module types, mapping cleanly to rustc compiler flags."
                </div>
            </div>

            <div class="feature-card">
                <div class="feature-icon">"📁"</div>
                <div class="feature-title">"Colocated Overrides"</div>
                <div class="feature-body">
                    "Place overrides.nix next to any Cargo.toml for surgical fileset filtering, system libraries (openssl, zlib), and custom environment variables."
                </div>
            </div>

            <div class="feature-card">
                <div class="feature-icon">"🔨"</div>
                <div class="feature-title">"Modern Build Scripts"</div>
                <div class="feature-body">
                    "Runs build.rs in an isolated sandbox, propagating both legacy cargo: and modern cargo:: directives and sys-crate DEP_* link flags."
                </div>
            </div>
        </div>
    })
}
