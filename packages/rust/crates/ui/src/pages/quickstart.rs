use bloomery_content::QUICKSTART;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/quickstart")]
pub async fn quickstart() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &QUICKSTART, active_path: "/docs/quickstart")
    })
}

#[page("/docs/quickstart")]
pub async fn docs_quickstart() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &QUICKSTART, active_path: "/docs/quickstart")
    })
}
