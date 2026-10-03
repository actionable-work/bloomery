use bloomery_content::PROFILES;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/profiles")]
pub async fn profiles() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &PROFILES, active_path: "/profiles")
    })
}

#[page("/docs/profiles")]
pub async fn docs_profiles() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &PROFILES, active_path: "/profiles")
    })
}
