use bloomery_content::BENCHMARKS;
use topcoat::Error;
use topcoat::router::page;
use topcoat::view::{View, view};

use crate::components::doc_page;

#[page("/benchmarks")]
pub async fn benchmarks() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &BENCHMARKS, active_path: "/benchmarks")
    })
}

#[page("/docs/benchmarks")]
pub async fn docs_benchmarks() -> Result<impl View, Error> {
    Ok(view! {
        doc_page(page: &BENCHMARKS, active_path: "/benchmarks")
    })
}
