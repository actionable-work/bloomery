use bloomery_content::{DocPage, render_markdown};
use topcoat::{
    Result,
    view::{Unescaped, View, component, view},
};

use crate::components::layout::page_layout;

#[component]
pub async fn doc_view(page: &'static DocPage) -> Result<impl View> {
    let rendered_html = render_markdown(page.markdown);

    Ok(view! {
        <article class="doc-article">
            <div class="doc-header" style="margin-bottom: 2.25rem;">
                <div class="pill forge" style="margin-bottom: 0.75rem;">(page.category)</div>
                <h1 class="doc-heading doc-h1" style="margin-top: 0;">(page.title)</h1>
                <p class="doc-paragraph" style="font-size: 1.15rem; color: var(--text-muted);">(page.description)</p>
            </div>

            <div class="doc-body">
                (Unescaped::new_unchecked(rendered_html))
            </div>

            <footer class="doc-footer">
                <span class="doc-footer-brand">"Bloomery Documentation"</span>
                <span class="doc-footer-meta">"Pure Nix Rust Engine • Zero Cargo"</span>
            </footer>
        </article>
    })
}

#[component]
pub async fn doc_page(page: &'static DocPage, active_path: &str) -> Result<impl View> {
    Ok(view! {
        page_layout(
            title: page.title,
            active_path: active_path,
            doc_view(page: page)
        )
    })
}
