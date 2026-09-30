use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn hero_banner() -> Result<impl View> {
    Ok(view! {
        <div class="hero-container" id="overview">
            <div class="hero-glow-1"></div>
            <div class="hero-glow-2"></div>

            <h1 class="hero-title">
                "Fast, Hermetic Rust Builds with Nix"
            </h1>

            <p class="hero-description">
                "Bloomery transforms standard "
                <code>"Cargo.lock"</code>
                " dependencies into isolated, reproducible derivation trees—delivering instant CI layer caching, fine-grained vendor hashing, and zero-drift cross-compilation without boilerplate generation."
            </p>

            <div class="hero-actions">
                <a href="/docs/quickstart" class="btn-primary">
                    <span class="material-symbols-outlined" style="font-size: 18px;">"terminal"</span>
                    <span>"Get Started"</span>
                </a>
                <a href="/benchmarks" class="btn-secondary">
                    <span class="material-symbols-outlined" style="font-size: 18px;">"query_stats"</span>
                    <span>"Benchmarks"</span>
                </a>
                <button class="copy-cmd-pill" id="copy-nix-eval" data-copy="nix flake check" title="Copy CLI check command">
                    <span class="material-symbols-outlined" style="font-size: 16px; color: var(--secondary);">"content_copy"</span>
                    <code>"nix flake check"</code>
                </button>
            </div>
        </div>
    })
}
