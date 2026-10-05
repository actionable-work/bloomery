//! Pages and assets for the website.

use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

/// The home page title.
pub const HOME_TITLE: &str = "{{project}}";

#[page("/")]
pub async fn home() -> Result<impl View, Error> {
    Ok(view! {
        <!doctype html>
        <html>
            <head>
                <title>(HOME_TITLE)</title>
            </head>
            <body>
                <h1>(HOME_TITLE)</h1>
            </body>
        </html>
    })
}

#[cfg(test)]
mod tests {
    #[cfg_attr(any(), bloomery("SITE-UI-PAGES-001"))]
    #[test]
    fn home_title_is_set() {
        assert_eq!(super::HOME_TITLE, "{{project}}");
    }
}
