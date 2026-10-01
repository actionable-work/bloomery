use bloomery_content::MATRIX;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/matrix")]
pub async fn matrix() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &MATRIX, active_path: "/matrix")
    })
}

#[page("/docs/matrix")]
pub async fn docs_matrix() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &MATRIX, active_path: "/matrix")
    })
}
