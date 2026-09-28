use bloomery_content::{DocPage, render_markdown};
use topcoat::{
    Result,
    view::{Unescaped, View, component, view},
};

use crate::components::card::feedback_box;
use crate::components::layout::page_layout;

#[component]
pub async fn doc_view(page: &'static DocPage) -> Result<impl View> {
    let rendered_html = render_markdown(page.markdown);

    let edit_url = format!(
        "https://github.com/actionable/bloomery/blob/main/packages/rust/crates/content/src/docs/{}.md",
        page.slug
    );

    Ok(view! {
        <article class="doc-article">
            <div class="doc-meta-bar">
                <div class="breadcrumbs">
                    <a href="/" class="breadcrumb-link" style="display: flex; align-items: center; gap: 0.25rem;">
                        <span class="material-symbols-outlined" style="font-size: 15px;">"folder"</span>
                        <span>"Docs"</span>
                    </a>
                    <span class="breadcrumb-sep">"/"</span>
                    <span>(page.category)</span>
                    <span class="breadcrumb-sep">"/"</span>
                    <span class="breadcrumb-active">(page.title)</span>
                </div>
                <div class="meta-badges">
                    <span class="meta-pill">
                        <span class="material-symbols-outlined" style="font-size: 14px;">"schedule"</span>
                        "5 min read"
                    </span>
                    <a href=(edit_url) target="_blank" rel="noopener noreferrer" class="meta-edit-btn" title="Edit source markdown on GitHub">
                        <span class="material-symbols-outlined" style="font-size: 14px;">"edit"</span>
                        "Edit"
                    </a>
                </div>
            </div>

            <div class="doc-header">
                <span class="pill forge" style="align-self: flex-start;">(page.category)</span>
                <h1 class="doc-h1">(page.title)</h1>
                <p class="doc-paragraph" style="font-size: 1.15rem; color: var(--text-muted); margin: 0;">(page.description)</p>
            </div>

            <div class="doc-body">
                (Unescaped::new_unchecked(rendered_html))
            </div>

            feedback_box()
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
