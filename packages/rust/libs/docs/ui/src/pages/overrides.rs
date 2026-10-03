use bloomery_content::OVERRIDES;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/overrides")]
pub async fn overrides() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &OVERRIDES, active_path: "/overrides")
    })
}

#[page("/docs/overrides")]
pub async fn docs_overrides() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &OVERRIDES, active_path: "/overrides")
    })
}
