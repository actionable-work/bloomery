use topcoat::{
    Result,
    view::{Child, View, component, view},
};

use crate::assets::BLOOMERY_CSS;
use crate::components::nav::sidebar;

#[component]
pub async fn page_layout(
    title: &str,
    active_path: &str,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let full_title = if title.is_empty() {
        "bloomery — Pure Nix Rust Engine".to_string()
    } else {
        format!("{title} — bloomery")
    };

    Ok(view! {
        <html lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <title>(full_title)</title>
                <link rel="stylesheet" href=(BLOOMERY_CSS) />
            </head>
            <body>
                sidebar(active_path: active_path)
                <main class="bloomery-content">
                    (child)
                </main>
            </body>
        </html>
    })
}
