use bloomery_content::{CONFIG, DocPage, all_pages, render_markdown};
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
        "{}/blob/main/packages/rust/crates/content/src/docs/{}.md",
        CONFIG.repo_url, page.slug
    );

    let all = all_pages();
    let current_idx = all.iter().position(|p| p.slug == page.slug);
    let prev_page = current_idx.and_then(|i| if i > 0 { Some(all[i - 1]) } else { None });
    let next_page = current_idx.and_then(|i| {
        if i + 1 < all.len() {
            Some(all[i + 1])
        } else {
            None
        }
    });

    let mut pagination_html = String::from(
        "<div class=\"doc-pagination\" style=\"display: flex; justify-content: space-between; align-items: center; margin-top: 3rem; padding-top: 1.5rem; border-top: 1px solid var(--border-color); gap: 1rem;\">",
    );
    if let Some(prev) = prev_page {
        let prev_href = if prev.slug == "overview" {
            "/"
        } else {
            prev.slug
        };
        pagination_html.push_str(&format!(
            "<a href=\"/{prev_href}\" class=\"pagination-btn prev\" style=\"display: flex; align-items: center; gap: 0.75rem; text-decoration: none; padding: 0.75rem 1.25rem; border-radius: 8px; border: 1px solid var(--border-color); background: var(--bg-card); color: var(--text-color); transition: all 0.2s;\"><span class=\"material-symbols-outlined\" style=\"font-size: 20px; color: var(--primary);\">arrow_back</span><div style=\"display: flex; flex-direction: column;\"><span style=\"font-size: 0.75rem; color: var(--text-dim); text-transform: uppercase; font-weight: 600;\">Previous</span><span style=\"font-size: 0.95rem; font-weight: 600;\">{}</span></div></a>",
            prev.title
        ));
    } else {
        pagination_html.push_str("<div></div>");
    }
    if let Some(next) = next_page {
        pagination_html.push_str(&format!(
            "<a href=\"/{}\" class=\"pagination-btn next\" style=\"display: flex; align-items: center; gap: 0.75rem; text-decoration: none; padding: 0.75rem 1.25rem; border-radius: 8px; border: 1px solid var(--border-color); background: var(--bg-card); color: var(--text-color); transition: all 0.2s;\"><div style=\"display: flex; flex-direction: column; text-align: right;\"><span style=\"font-size: 0.75rem; color: var(--text-dim); text-transform: uppercase; font-weight: 600;\">Next</span><span style=\"font-size: 0.95rem; font-weight: 600;\">{}</span></div><span class=\"material-symbols-outlined\" style=\"font-size: 20px; color: var(--primary);\">arrow_forward</span></a>",
            next.slug,
            next.title
        ));
    }
    pagination_html.push_str("</div>");

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
                <h1 class="doc-h1">(page.title)</h1>
                <p class="doc-paragraph" style="font-size: 1.15rem; color: var(--text-muted); margin: 0;">(page.description)</p>
            </div>

            <div class="doc-body">
                (Unescaped::new_unchecked(rendered_html))
            </div>

            (Unescaped::new_unchecked(pagination_html))

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
