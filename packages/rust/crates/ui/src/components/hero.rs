use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn hero_banner() -> Result<impl View> {
    Ok(view! {
        <div class="hero-container">
            <div class="hero-pill-bar">
                <span class="pill forge">"🔥 Pure Nix Rust Engine"</span>
                <span class="pill emerald">"✓ Zero Cargo Overhead"</span>
                <span class="pill cyan">"⚡ Hermetic Store Caching"</span>
                <span class="pill">"🦀 Built with Topcoat"</span>
            </div>
            <h1 class="hero-title">
                "Forge pure Rust binaries straight from Cargo.lock."
            </h1>
            <p class="hero-description">
                "Every crate becomes an independent Nix store derivation. Rebuild leaves in seconds with zero cache invalidation, parallel check runners, and typed optimization profiles."
            </p>
            <div class="hero-actions">
                <a href="/docs/quickstart" class="btn-primary">
                    <span>"Get Started"</span>
                    <span>"→"</span>
                </a>
                <a href="/api" class="btn-secondary">
                    <span>"API Reference"</span>
                </a>
                <a href="/profiles" class="btn-secondary">
                    <span>"Profile Options"</span>
                </a>
            </div>
        </div>
    })
}
