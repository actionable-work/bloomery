use bloomery_content::ARCHITECTURE;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/architecture")]
pub async fn architecture() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &ARCHITECTURE, active_path: "/docs/architecture")
    })
}

#[page("/docs/architecture")]
pub async fn docs_architecture() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &ARCHITECTURE, active_path: "/docs/architecture")
    })
}
