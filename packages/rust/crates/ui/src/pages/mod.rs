pub mod api;
pub mod architecture;
pub mod benchmarks;
pub mod home;
pub mod matrix;
pub mod overrides;
pub mod overview;
pub mod profiles;
pub mod quickstart;

pub use api::*;
pub use architecture::*;
pub use benchmarks::*;
pub use home::*;
pub use matrix::*;
pub use overrides::*;
pub use overview::*;
pub use profiles::*;
pub use quickstart::*;

use topcoat::router::RouterBuilder;

/// Register all documentation and application pages onto a Topcoat router builder.
pub fn register_pages(builder: RouterBuilder) -> RouterBuilder {
    builder
        .page(home)
        .page(overview)
        .page(docs_overview)
        .page(quickstart)
        .page(docs_quickstart)
        .page(architecture)
        .page(docs_architecture)
        .page(profiles)
        .page(docs_profiles)
        .page(api)
        .page(docs_api)
        .page(overrides)
        .page(docs_overrides)
        .page(matrix)
        .page(docs_matrix)
        .page(benchmarks)
        .page(docs_benchmarks)
}
