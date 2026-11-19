//! Pages and assets for the site.

use topcoat::Error;
use topcoat::asset::{Asset, asset};
use topcoat::router::page;
use topcoat::tailwind;
use topcoat::view::{View, view};

/// The UI stylesheet, sharing its file name with the brand stylesheet.
pub const STYLE: Asset = asset!("assets/site.css");

/// The home page title.
pub const HOME_TITLE: &str = "site";

#[page("/")]
pub async fn home() -> Result<impl View, Error> {
    Ok(view! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>(HOME_TITLE)</title>
                <link rel="stylesheet" href=(STYLE)>
                <link rel="stylesheet" href=(site_brand::STYLE)>
                <link rel="stylesheet" href=(tailwind::stylesheet!())>
            </head>
            <body>
                <h1 class="text-center bg-red-500 underline">(HOME_TITLE)</h1>
            </body>
        </html>
    })
}
