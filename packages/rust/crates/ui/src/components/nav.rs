use bloomery_content::categories;
use topcoat::{
    Result,
    view::{View, component, view},
};

use crate::assets::BLOOMERY_LOGO;

#[component]
pub async fn sidebar(active_path: &str) -> Result<impl View> {
    let cats = categories();

    Ok(view! {
        <aside class="bloomery-sidebar">
            <a href="/" class="brand-wrapper">
                <img src=(BLOOMERY_LOGO) class="brand-logo-img" alt="Bloomery Forge" />
                <span class="brand-title">
                    "bloomery"
                    <span class="brand-badge">"v0.1.0"</span>
                </span>
            </a>

            <nav class="sidebar-nav">
                for cat in cats {
                    <div class="sidebar-category">
                        <div class="category-title">
                            <span>(cat.icon)</span>
                            <span>(cat.name)</span>
                        </div>
                        <ul>
                            for page in cat.pages {
                                let href = if page.slug == "overview" {
                                    "/".to_string()
                                } else {
                                    format!("/docs/{}", page.slug)
                                };
                                let is_active = active_path == href || (active_path == "/" && page.slug == "overview");
                                let active_class = if is_active { "active" } else { "" };
                                <li>
                                    <a href=(href) class=(active_class)>
                                        <span>(page.title)</span>
                                    </a>
                                </li>
                            }
                        </ul>
                    </div>
                }

                <div class="sidebar-category">
                    <div class="category-title">
                        <span>"🧪"</span>
                        <span>"Reference & Overrides"</span>
                    </div>
                    <ul>
                        <li>
                            <a href="/profiles" class=(if active_path == "/profiles" { "active" } else { "" })>
                                <span>"Strongly-Typed Profiles"</span>
                            </a>
                        </li>
                        <li>
                            <a href="/api" class=(if active_path == "/api" { "active" } else { "" })>
                                <span>"mkWorkspace Options API"</span>
                            </a>
                        </li>
                        <li>
                            <a href="/overrides" class=(if active_path == "/overrides" { "active" } else { "" })>
                                <span>"Colocated overrides.nix"</span>
                            </a>
                        </li>
                    </ul>
                </div>
            </nav>

            <div class="sidebar-footer">
                <div class="cmd-badge" onclick="navigator.clipboard.writeText('nix run github:actionable/bloomery#lock')">
                    <span>"$ nix run .#lock"</span>
                    <span>"📋"</span>
                </div>
                <a href="https://github.com/actionable/bloomery" target="_blank" rel="noopener noreferrer" class="doc-link" style="font-size: 0.85rem; text-align: center;">
                    "GitHub Repository ↗"
                </a>
            </div>
        </aside>
    })
}
