use bloomery_content::OVERVIEW;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/overview")]
pub async fn overview() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &OVERVIEW, active_path: "/docs/overview")
    })
}

#[page("/docs/overview")]
pub async fn docs_overview() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &OVERVIEW, active_path: "/docs/overview")
    })
}
