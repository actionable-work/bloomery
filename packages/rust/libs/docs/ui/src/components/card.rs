use topcoat::{
    Result,
    view::{Unescaped, View, component, view},
};

const FLAKE_CODE_HTML: &str = r#"<span class="syntax-punct">{</span>
  <span class="syntax-kw">inputs</span> = <span class="syntax-punct">{</span>
    <span class="syntax-fn">nixpkgs</span>.url = <span class="syntax-str">"github:NixOS/nixpkgs/nixos-unstable"</span>;
    <span class="syntax-fn">bloomery</span>.url = <span class="syntax-str">"github:actionable-work/bloomery"</span>;
  <span class="syntax-punct">}</span>;

  <span class="syntax-kw">outputs</span> = <span class="syntax-punct">{</span> nixpkgs, bloomery, ... <span class="syntax-punct">}</span>:
    bloomery.mkFlake <span class="syntax-punct">{</span>
      <span class="syntax-kw">inherit</span> nixpkgs;
      root = ./.;
    <span class="syntax-punct">}</span>;
<span class="syntax-punct">}</span>"#;

#[component]
pub async fn feature_cards() -> Result<impl View> {
    Ok(view! {
        <div class="grid-features" id="key-features">
            <div class="feature-card">
                <div class="feature-icon-wrapper primary">
                    <span class="material-symbols-outlined">"folder_special"</span>
                </div>
                <h3 class="feature-title">"Zero-Config Cargo"</h3>
                <p class="feature-body">
                    "Consumes Cargo workspace trees directly. No "
                    <code>"cargo2nix"</code>
                    " JSON files or manual vendor lockfile syncs required during day-to-day development."
                </p>
            </div>

            <div class="feature-card">
                <div class="feature-icon-wrapper secondary">
                    <span class="material-symbols-outlined">"layers"</span>
                </div>
                <h3 class="feature-title">"Incremental Derivations"</h3>
                <p class="feature-body">
                    "Split vendor crates from user code to hit Cachix, Attic, or Hydra caches effortlessly. Rebuild only what actually changed in your commit graph."
                </p>
            </div>

            <div class="feature-card">
                <div class="feature-icon-wrapper tertiary">
                    <span class="material-symbols-outlined">"devices"</span>
                </div>
                <h3 class="feature-title">"Cross-Platform Matrices"</h3>
                <p class="feature-body">
                    "Clean Tier 1 cross-build matrices for Linux, musl statically linked targets, and macOS Apple Silicon without container orchestration overhead."
                </p>
            </div>

            <div class="feature-card">
                <div class="feature-icon-wrapper primary">
                    <span class="material-symbols-outlined">"terminal"</span>
                </div>
                <h3 class="feature-title">"Flake Native"</h3>
                <p class="feature-body">
                    "Standardized outputs with bundled devShells, "
                    <code>"rust-analyzer"</code>
                    ", and "
                    <code>"clippy"</code>
                    " pre-configured and pinned hermetically."
                </p>
            </div>
        </div>
    })
}

#[component]
pub async fn code_tabs_showcase() -> Result<impl View> {
    Ok(view! {
        <section style="display: flex; flex-direction: column; gap: 1rem;" id="installation">
            <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                <span class="pill forge" style="align-self: flex-start;">"Integration"</span>
                <h2 class="doc-h2" style="margin: 0.25rem 0; border: none; padding: 0;">"Quickstart & Installation"</h2>
                <p class="doc-paragraph" style="margin: 0; color: var(--text-muted);">
                    "Add Bloomery as an input to your existing "
                    <code style="color: var(--secondary);">"flake.nix"</code>
                    ". It integrates immediately with standard Nix CLI tools."
                </p>
            </div>

            <div class="code-window">
                <div class="code-tab-bar">
                    <div class="tab-btn-group">
                        <button class="tab-btn active" data-tab="flake">
                            <span class="tab-dot"></span>
                            "flake.nix"
                        </button>
                        <button class="tab-btn" data-tab="overrides">
                            "overrides.nix"
                        </button>
                        <button class="tab-btn" data-tab="devshell">
                            "devShell"
                        </button>
                    </div>
                    <div style="display: flex; align-items: center; gap: 0.75rem;">
                        <span style="font-family: var(--font-mono); font-size: 0.75rem; color: var(--text-dim);">"Nix 2.18+"</span>
                        <button class="code-copy-btn" id="copy-code-btn" data-copy="nix flake check" title="Copy code snippet">
                            <span class="material-symbols-outlined" style="font-size: 16px;">"content_copy"</span>
                        </button>
                    </div>
                </div>

                <div class="code-content-pane">
                    <div class="line-numbers">
                        <div>"1"</div><div>"2"</div><div>"3"</div><div>"4"</div><div>"5"</div>
                        <div>"6"</div><div>"7"</div><div>"8"</div><div>"9"</div><div>"10"</div>
                        <div>"11"</div><div>"12"</div><div>"13"</div><div>"14"</div>
                    </div>
                    <pre class="code-block">(Unescaped::new_unchecked(FLAKE_CODE_HTML))</pre>
                </div>

                <div class="terminal-preview">
                    <div class="terminal-header">
                        <div class="terminal-dots">
                            <span class="dot red"></span>
                            <span class="dot yellow"></span>
                            <span class="dot green"></span>
                            <span style="font-weight: 700; margin-left: 0.4rem; color: var(--text-muted);">"Terminal Execution"</span>
                        </div>
                        <span style="opacity: 0.5;">"sandboxed"</span>
                    </div>
                    <div class="terminal-prompt-line">
                        <span class="terminal-prompt-sym">"$"</span>
                        <span>"nix build .# --print-build-logs"</span>
                    </div>
                    <div class="terminal-logs">
                        <div>"[1/4] vendoring cargo crates (cached)... "<span style="color: var(--secondary);">"1.4s ready"</span></div>
                        <div>"[2/4] evaluating workspace profile [release]"</div>
                        <div>"[3/4] compiling "<span style="color: var(--primary); font-weight: 600;">"tokio v1.38.0"</span>" via nix-store derivation"</div>
                        <div class="terminal-success">
                            <span class="material-symbols-outlined" style="font-size: 14px;">"check_circle"</span>
                            "[4/4] built derivation -> /nix/store/29df...-bloomery-0.1.0"
                        </div>
                    </div>
                </div>
            </div>
        </section>
    })
}

#[component]
pub async fn architecture_diagram_card() -> Result<impl View> {
    Ok(view! {
        <div class="architecture-card" id="graph">
            <div class="architecture-text">
                <div style="display: flex; align-items: center; gap: 0.4rem; color: var(--secondary); font-size: 0.775rem; font-weight: 700; text-transform: uppercase;">
                    <span class="material-symbols-outlined" style="font-size: 16px;">"account_tree"</span>
                    "Derivation Graph Flow"
                </div>
                <h3 class="feature-title">"Deterministic Workspace Slicing"</h3>
                <p class="feature-body">
                    "Bloomery isolates 3rd-party dependencies from user source revisions. Third-party crates compile once to immutable derivations, making branch switching instant."
                </p>
            </div>

            <div class="architecture-diagram">
                <svg viewBox="0 0 320 120" style="width: 100%; height: 100%;" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <rect x="10" y="42" width="70" height="36" rx="4" fill="#262a34" />
                    <text x="45" y="64" fill="#dfe2ef" font-family="monospace" font-size="9" font-weight="bold" text-anchor="middle">"Cargo.lock"</text>

                    <path d="M80 60 H115" stroke="#7bd0ff" stroke-width="1.5" stroke-dasharray="3 3" />
                    <path d="M115 60 V28 H145" stroke="#7bd0ff" stroke-width="1.5" />
                    <path d="M115 60 V92 H145" stroke="#f97316" stroke-width="1.5" />

                    <rect x="145" y="10" width="75" height="36" rx="4" fill="#00374d" />
                    <text x="182" y="32" fill="#7bd0ff" font-family="monospace" font-size="8" text-anchor="middle">"Vendor Deriv"</text>

                    <rect x="145" y="74" width="75" height="36" rx="4" fill="#582200" />
                    <text x="182" y="96" fill="#ffb690" font-family="monospace" font-size="8" text-anchor="middle">"Local Crate"</text>

                    <path d="M220 28 H245 V60" stroke="#7bd0ff" stroke-width="1.5" />
                    <path d="M220 92 H245 V60" stroke="#f97316" stroke-width="1.5" />
                    <path d="M245 60 H260" stroke="#dfe2ef" stroke-width="1.5" />

                    <rect x="260" y="42" width="50" height="36" rx="4" fill="#1c1f29" stroke="#7bd0ff" stroke-width="1" />
                    <text x="285" y="64" fill="#dfe2ef" font-family="monospace" font-size="8" text-anchor="middle">"bin"</text>
                </svg>
            </div>
        </div>
    })
}

#[component]
pub async fn metrics_banner() -> Result<impl View> {
    Ok(view! {
        <section style="display: flex; flex-direction: column; gap: 1.25rem;" id="benchmarks">
            <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                <span class="pill cyan" style="align-self: flex-start;">"Architectural Advantage"</span>
                <h2 class="doc-h2" style="margin: 0.25rem 0; border: none; padding: 0;">"Why Bloomery? Compared to Crane & Cargo2nix"</h2>
                <p class="doc-paragraph" style="margin: 0; color: var(--text-muted);">
                    "Engineered to resolve the common pain points of existing Nix-Rust solutions: slow cache thrashing, fragile JSON lock synchronizations, and unmanageable derivations."
                </p>
            </div>

            <div class="grid-metrics">
                <div class="metric-card">
                    <div class="metric-top">
                        <span>"CI Build Time"</span>
                        <span class="material-symbols-outlined" style="color: var(--primary); font-size: 20px;">"speed"</span>
                    </div>
                    <span class="metric-value primary">"-42%"</span>
                    <span class="metric-desc">"Average reduction in cold GitHub Actions runs"</span>
                </div>

                <div class="metric-card">
                    <div class="metric-top">
                        <span>"Cachix Hit Rate"</span>
                        <span class="material-symbols-outlined" style="color: var(--secondary); font-size: 20px;">"hub"</span>
                    </div>
                    <span class="metric-value secondary">"94%"</span>
                    <span class="metric-desc">"Shared layer reuse across monorepo crates"</span>
                </div>

                <div class="metric-card">
                    <div class="metric-top">
                        <span>"Code Overhead"</span>
                        <span class="material-symbols-outlined" style="color: var(--tertiary); font-size: 20px;">"code_blocks"</span>
                    </div>
                    <span class="metric-value tertiary">"0 LoC"</span>
                    <span class="metric-desc">"No generated Nix scaffolding checked into git"</span>
                </div>
            </div>

            <div style="display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 1rem;">
                <div class="feature-card" style="padding: 1.25rem;">
                    <div style="display: flex; align-items: center; gap: 0.5rem; color: var(--primary); font-weight: 700; font-size: 0.95rem;">
                        <span class="material-symbols-outlined" style="font-size: 18px;">"verified"</span>
                        "Fine-Grained Derivations"
                    </div>
                    <p style="font-size: 0.85rem; color: var(--text-muted); line-height: 1.55;">
                        "Heavyweight crates like tokio, syn, or serde compile exactly once across team machines and remote CI builders."
                    </p>
                </div>

                <div class="feature-card" style="padding: 1.25rem;">
                    <div style="display: flex; align-items: center; gap: 0.5rem; color: var(--secondary); font-weight: 700; font-size: 0.95rem;">
                        <span class="material-symbols-outlined" style="font-size: 18px;">"shield"</span>
                        "No Drift Risks"
                    </div>
                    <p style="font-size: 0.85rem; color: var(--text-muted); line-height: 1.55;">
                        "Eliminates intermediate generated files slipping out of sync with updated Cargo.lock dependencies."
                    </p>
                </div>

                <div class="feature-card" style="padding: 1.25rem;">
                    <div style="display: flex; align-items: center; gap: 0.5rem; color: var(--tertiary); font-weight: 700; font-size: 0.95rem;">
                        <span class="material-symbols-outlined" style="font-size: 18px;">"fingerprint"</span>
                        "Cryptographic SRI Hashes"
                    </div>
                    <p style="font-size: 0.85rem; color: var(--text-muted); line-height: 1.55;">
                        "Automatic deterministic SRI hashes for git and registry dependencies directly extracted during evaluation."
                    </p>
                </div>
            </div>
        </section>
    })
}

#[component]
pub async fn next_steps_cards() -> Result<impl View> {
    Ok(view! {
        <section style="display: flex; flex-direction: column; gap: 1rem;" id="next-steps">
            <div style="display: flex; flex-direction: column; gap: 0.25rem;">
                <span class="pill" style="align-self: flex-start;">"Recommended Paths"</span>
                <h2 class="doc-h2" style="margin: 0.25rem 0; border: none; padding: 0;">"Next Steps"</h2>
            </div>

            <div class="grid-next-steps">
                <a href="/docs/architecture" class="next-card">
                    <div class="next-card-top">
                        <div class="next-card-meta primary">
                            <span>"Architecture Guide"</span>
                            <span class="material-symbols-outlined" style="font-size: 18px;">"arrow_forward"</span>
                        </div>
                        <h4 class="next-card-title">"Configuring Workspaces"</h4>
                        <p class="next-card-desc">
                            "Manage complex multi-crate Rust workspaces, individual package build targets, and conditional feature flags cleanly."
                        </p>
                    </div>
                    <div class="next-card-footer">
                        <span>"Guide 01"</span>
                        <span>"•"</span>
                        <span>"3 min read"</span>
                    </div>
                </a>

                <a href="/profiles" class="next-card">
                    <div class="next-card-top">
                        <div class="next-card-meta secondary">
                            <span>"Production Guide"</span>
                            <span class="material-symbols-outlined" style="font-size: 18px;">"arrow_forward"</span>
                        </div>
                        <h4 class="next-card-title">"Strongly-Typed Profiles"</h4>
                        <p class="next-card-desc">
                            "Target musl statically linked binaries, thin/fat LTO, custom codegen units, and compiler flags with full Nix validation."
                        </p>
                    </div>
                    <div class="next-card-footer">
                        <span>"Guide 02"</span>
                        <span>"•"</span>
                        <span>"5 min read"</span>
                    </div>
                </a>
            </div>
        </section>
    })
}

use bloomery_content::CONFIG;

#[component]
pub async fn feedback_box() -> Result<impl View> {
    Ok(view! {
        <div class="feedback-box">
            <div class="feedback-text">
                <span class="feedback-title">"Was this page helpful?"</span>
                <span class="feedback-sub">"Let us know how we can improve these docs."</span>
            </div>
            <div class="feedback-actions" id="feedback-container">
                <button class="feedback-btn" id="feedback-yes" title="Yes, helpful">
                    <span class="material-symbols-outlined" style="font-size: 16px; color: var(--secondary);">"thumb_up"</span>
                    <span>"Yes"</span>
                </button>
                <button class="feedback-btn" id="feedback-no" title="Needs improvement">
                    <span class="material-symbols-outlined" style="font-size: 16px; color: var(--error);">"thumb_down"</span>
                    <span>"No"</span>
                </button>
                <span style="opacity: 0.3; margin: 0 0.25rem;">"|"</span>
                <a href=(CONFIG.issues_url) target="_blank" rel="noopener noreferrer" style="font-size: 0.85rem; color: var(--text-dim); display: flex; align-items: center; gap: 0.35rem;">
                    <span class="material-symbols-outlined" style="font-size: 16px;">"bug_report"</span>
                    <span>"Report issue"</span>
                </a>
            </div>
        </div>
    })
}
