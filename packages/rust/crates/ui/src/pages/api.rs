use bloomery_content::API;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/api")]
pub async fn api() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &API, active_path: "/api")
    })
}

#[page("/docs/api")]
pub async fn docs_api() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &API, active_path: "/api")
    })
}
